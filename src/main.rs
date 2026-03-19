use anyhow::Result;
use rmcp::{ServerHandler, model::*, service::RequestContext, tool};
use serde_json::{json, Value};
use tracing::info;

mod agent;
mod cache;
mod scanner;

// ── MCP Server ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct ArchGuardServer;

#[rmcp::tool(description = "Scan a project directory and build its architecture model. \
    Call this once when opening a project. The model is cached automatically.")]
async fn scan_project(
    // Absolute path to the project root directory
    root_dir: String,
) -> Value {
    info!("scan_project: {}", root_dir);
    let path = std::path::Path::new(&root_dir);

    if !path.exists() {
        return json!({ "error": format!("directory not found: {root_dir}") });
    }

    // Return cached model if directory structure unchanged
    if let Ok(Some(cached)) = cache::get(&root_dir) {
        info!("scan_project: cache hit for {}", root_dir);
        return json!({ "status": "cached", "model": cached });
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
            json!({ "status": "ok", "model": model })
        }
        Err(e) => {
            tracing::error!("scan_project failed: {}", e);
            json!({ "error": e.to_string() })
        }
    }
}

#[rmcp::tool(description = "Review a single file for architectural violations. \
    Requires scan_project to have been called first. Returns structured findings \
    with severity, explanation, consequence, and suggested fix.")]
async fn review_file(
    // Absolute path to the project root (same as used in scan_project)
    root_dir: String,
    // Path to the file being reviewed (absolute or relative to root_dir)
    file_path: String,
    // Full content of the file to review
    content: String,
) -> Value {
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
                    return json!({ "error": format!("could not build arch model: {e}") });
                }
            }
        }
    };

    match agent::review(&model, &file_path, &content).await {
        Ok(findings) => {
            info!("review_file: {} finding(s) for {}", findings.len(), file_path);
            json!({ "findings": findings })
        }
        Err(e) => {
            tracing::error!("review_file failed: {}", e);
            json!({ "error": e.to_string() })
        }
    }
}

#[rmcp::tool(description = "Clear the cached architecture model for a project. \
    Use this after major refactors to force a fresh scan.")]
async fn invalidate_cache(
    // Absolute path to the project root
    root_dir: String,
) -> Value {
    match cache::invalidate(&root_dir) {
        Ok(_) => json!({ "status": "cache cleared", "root": root_dir }),
        Err(e) => json!({ "error": e.to_string() }),
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    // Log to stderr — stdout is reserved for MCP JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            std::env::var("ARCHGUARD_LOG")
                .unwrap_or_else(|_| "archguard_mcp=info".to_string())
        )
        .init();

    info!("archguard-mcp starting (v{})", env!("CARGO_PKG_VERSION"));

    // Validate API key is set before accepting connections
    if std::env::var("ARCHGUARD_API_KEY").is_err() {
        eprintln!("ERROR: ARCHGUARD_API_KEY environment variable not set.");
        eprintln!("Get your API key at https://resolvingarchitecture.io/archguard/dashboard");
        std::process::exit(1);
    }

    let server = ArchGuardServer;

    // Stdio transport — Cursor/Claude Code spawn this process
    rmcp::serve_stdio(server).await?;
    Ok(())
}
