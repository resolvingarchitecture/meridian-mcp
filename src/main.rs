use anyhow::Result;
use rmcp::{
    handler::server::tool::Parameters,
    model::ServerInfo,
    schemars, tool, ServerHandler,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::info;

mod agent;
mod cache;
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

    info!("meridian starting in MCP mode (v{})", env!("CARGO_PKG_VERSION"));

    // Validate API key is set before accepting connections
    if std::env::var("MERIDIAN_API_KEY").is_err() {
        eprintln!("ERROR: MERIDIAN_API_KEY environment variable not set.");
        eprintln!("Get your API key at https://resolvingarchitecture.io/meridian");
        std::process::exit(1);
    }

    let server = MeridianServer;

    // Stdio transport — Cursor/Claude Code spawn this process
    use rmcp::service::serve_server;
    use rmcp::transport::io::stdio;

    serve_server(server, stdio()).await?;
    Ok(())
}