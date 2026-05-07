use anyhow::{Context, Result};
use rmcp::{
    handler::server::tool::Parameters,
    model::ServerInfo,
    schemars, tool, ServerHandler,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::info;

mod agent;
mod cache;
mod config;
mod scanner;

// ── MCP Server ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MeridianServer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScanProjectRequest {
    root_dir: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReviewFileRequest {
    root_dir: String,
    file_path: String,
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct InvalidateCacheRequest {
    root_dir: String,
}

#[tool(tool_box)]
impl MeridianServer {
    #[tool(description = "Scan a Meridian project directory and build its architecture model. \
        Call this once when opening a project. The model is cached automatically.")]
    async fn scan_project(
        &self,
        Parameters(req): Parameters<ScanProjectRequest>,
    ) -> String {
        let root_dir = req.root_dir;
        info!("scan_project: {}", root_dir);
        let path = std::path::Path::new(&root_dir);

        if !path.exists() {
            return json!({ "error": format!("directory not found: {root_dir}") }).to_string();
        }

        // Return cached model if directory structure unchanged
        if let Ok(Some(cached)) = cache::get(&root_dir) {
            info!("scan_project: cache hit for {}", root_dir);
            return json!({ "status": "cached", "model": cached }).to_string();
        }

        match scanner::scan(path) {
            Ok(model) => {
                if let Err(e) = cache::set(&root_dir, &model) {
                    tracing::warn!("Failed to cache arch model: {}", e);
                }
                info!(
                    "scan_project: complete — style={}, layers={:?}",
                    model.style,
                    model.layer_order
                );
                json!({ "status": "ok", "model": model }).to_string()
            }
            Err(e) => {
                tracing::error!("scan_project failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(description = "Review a single file for architectural violations. \
        Requires scan_project to have been called first. Returns structured findings \
        with severity, explanation, consequence, and suggested fix.")]
    async fn review_file(
        &self,
        Parameters(req): Parameters<ReviewFileRequest>,
    ) -> String {
        let root_dir = req.root_dir;
        let file_path = req.file_path;
        let content = req.content;

        info!("review_file: {}", file_path);

        // Load cached arch model — scan first if not yet scanned
        let model = match cache::get(&root_dir) {
            Ok(Some(m)) => m,
            _ => {
                let path = std::path::Path::new(&root_dir);
                match scanner::scan(path) {
                    Ok(m) => {
                        let _ = cache::set(&root_dir, &m);
                        m
                    }
                    Err(e) => {
                        return json!({ "error": format!("could not build arch model: {e}") }).to_string();
                    }
                }
            }
        };

        match agent::review(&model, &file_path, &content).await {
            Ok(findings) => {
                info!("review_file: {} finding(s) for {}", findings.len(), file_path);
                json!({ "findings": findings }).to_string()
            }
            Err(e) => {
                tracing::error!("review_file failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(description = "Clear the cached architecture model for a project. \
        Use this after major refactors to force a fresh scan.")]
    async fn invalidate_cache(
        &self,
        Parameters(req): Parameters<InvalidateCacheRequest>,
    ) -> String {
        let root_dir = req.root_dir;

        match cache::invalidate(&root_dir) {
            Ok(_) => json!({ "status": "cache cleared", "root": root_dir }).to_string(),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }
}

impl ServerHandler for MeridianServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("Meridian local architecture review MCP server".into()),
            ..Default::default()
        }
    }
}

// ── CLI ──────────────────────────────────────────────────────────────────────

async fn run_cli(args: Vec<String>) -> Result<()> {
    match args.get(1).map(String::as_str) {
        None | Some("mcp") => run_mcp_server().await,
        Some("scan") => {
            let root = args.get(2).map(String::as_str).unwrap_or(".");
            cli_scan(root)
        }
        Some("review") => {
            let file_path = args
                .get(2)
                .context("usage: meridian review <file_path>")?;
            cli_review(file_path).await
        }
        Some("cache") => match args.get(2).map(String::as_str) {
            Some("clear") => {
                let root = args.get(3).map(String::as_str).unwrap_or(".");
                cli_cache_clear(root)
            }
            _ => {
                anyhow::bail!("usage: meridian cache clear [root_dir]");
            }
        },
        Some("config") => match (args.get(2).map(String::as_str), args.get(3).map(String::as_str)) {
            (Some("set"), Some("api-key")) => {
                let key = args
                    .get(4)
                    .context("usage: meridian config set api-key <key>")?;
                cli_config_set_api_key(key)
            }
            _ => {
                anyhow::bail!("usage: meridian config set api-key <key>");
            }
        },
        Some("doctor") => cli_doctor(),
        Some("version") | Some("--version") | Some("-V") => {
            println!("meridian {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some(command) => {
            anyhow::bail!("unknown command: {command}\n\nRun `meridian help` for usage.");
        }
    }
}

fn cli_scan(root: &str) -> Result<()> {
    let path = Path::new(root);

    if !path.exists() {
        anyhow::bail!("directory not found: {root}");
    }

    if !path.is_dir() {
        anyhow::bail!("not a directory: {root}");
    }

    if let Some(model) = cache::get(root)? {
        println!("{}", serde_json::to_string_pretty(&json!({
            "status": "cached",
            "model": model
        }))?);
        return Ok(());
    }

    let model = scanner::scan(path)
        .with_context(|| format!("failed to scan project: {root}"))?;

    cache::set(root, &model)
        .with_context(|| format!("failed to cache architecture model for: {root}"))?;

    println!("{}", serde_json::to_string_pretty(&json!({
        "status": "ok",
        "model": model
    }))?);

    Ok(())
}

async fn cli_review(file_path: &str) -> Result<()> {
    let file = PathBuf::from(file_path);

    if !file.exists() {
        anyhow::bail!("file not found: {file_path}");
    }

    if !file.is_file() {
        anyhow::bail!("not a file: {file_path}");
    }

    let content = std::fs::read_to_string(&file)
        .with_context(|| format!("failed to read file: {file_path}"))?;

    let root = std::env::current_dir()
        .context("failed to determine current directory")?;

    let root_str = root.to_string_lossy().to_string();

    let model = match cache::get(&root_str)? {
        Some(model) => model,
        None => {
            let model = scanner::scan(&root)
                .with_context(|| format!("failed to scan project: {}", root.display()))?;
            cache::set(&root_str, &model)?;
            model
        }
    };

    let findings = agent::review(&model, file_path, &content).await?;

    println!("{}", serde_json::to_string_pretty(&json!({
        "findings": findings
    }))?);

    Ok(())
}

fn cli_cache_clear(root: &str) -> Result<()> {
    cache::invalidate(root)
        .with_context(|| format!("failed to clear cache for: {root}"))?;

    println!("{}", serde_json::to_string_pretty(&json!({
        "status": "cache cleared",
        "root": root
    }))?);

    Ok(())
}

fn cli_config_set_api_key(key: &str) -> Result<()> {
    if !key.starts_with("m_live_") {
        eprintln!("warning: Meridian API keys usually start with `m_live_`");
    }

    config::set_api_key(key)?;

    println!("{}", serde_json::to_string_pretty(&json!({
        "status": "ok",
        "message": "API key saved",
        "config_file": config::config_file_display_path()
    }))?);

    Ok(())
}

fn cli_doctor() -> Result<()> {
    let env_api_key = std::env::var("MERIDIAN_API_KEY").ok();
    let configured_api_key = config::load()?.api_key;

    let api_key_status = if env_api_key.as_deref().is_some_and(|key| !key.trim().is_empty()) {
        "set via MERIDIAN_API_KEY"
    } else if configured_api_key.as_deref().is_some_and(|key| !key.trim().is_empty()) {
        "set in local config"
    } else {
        "missing"
    };

    let backend_url = std::env::var("MERIDIAN_BACKEND_URL")
        .unwrap_or_else(|_| "https://resolvingarchitecture.io/meridian/api".to_string());

    println!("{}", serde_json::to_string_pretty(&json!({
        "status": if api_key_status == "missing" { "warning" } else { "ok" },
        "version": env!("CARGO_PKG_VERSION"),
        "api_key": api_key_status,
        "backend_url": backend_url,
        "config_file": config::config_file_display_path()
    }))?);

    if api_key_status == "missing" {
        eprintln!("warning: no API key configured. Run: meridian config set api-key <key>");
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"Meridian local architecture review MCP server and CLI

Usage:
  meridian mcp
  meridian scan [root_dir]
  meridian review <file_path>
  meridian cache clear [root_dir]
  meridian config set api-key <key>
  meridian doctor
  meridian version
  meridian help

Environment:
  MERIDIAN_API_KEY      API key, usually m_live_...
  MERIDIAN_BACKEND_URL  Backend URL
  MERIDIAN_LOG          Log level for stderr logs
"#
    );
}

async fn run_mcp_server() -> Result<()> {
    info!("meridian starting in MCP mode (v{})", env!("CARGO_PKG_VERSION"));

    if crate::config::api_key().is_err() {
        eprintln!("ERROR: MERIDIAN_API_KEY environment variable not set and no local API key configured.");
        eprintln!("Run: meridian config set api-key <key>");
        eprintln!("Get your API key at https://resolvingarchitecture.io/meridian");
        std::process::exit(1);
    }

    let server = MeridianServer;

    use rmcp::service::serve_server;
    use rmcp::transport::io::stdio;

    serve_server(server, stdio()).await?;
    Ok(())
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Log to stderr — stdout is reserved for MCP JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            std::env::var("MERIDIAN_LOG")
                .unwrap_or_else(|_| "meridian=info".to_string())
        )
        .init();

    run_cli(std::env::args().collect()).await
}