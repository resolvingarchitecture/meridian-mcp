//! `meridian-mcp`: the agent-facing interface to Meridian (`ADR-AppArch-25`).
//!
//! A stateless client: stdio MCP toward the agent, authenticated HTTPS toward
//! `meridian-backend`. It holds no copy of the Architecture Model and makes no AI calls.

mod backend;
mod config;
mod review_report;
mod session;
mod tools;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};

#[derive(Parser)]
#[command(name = "meridian-mcp", version, about = "Meridian agent interface (MCP server and CLI)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the MCP server on stdio (the default).
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve().await,
    }
}

async fn serve() -> Result<()> {
    let backend = backend::Backend::new(config::Config::from_env()?)?;
    let service = tools::MeridianTools::new(backend).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
