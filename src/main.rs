//! `meridian-mcp`: the agent-facing interface to Meridian (`ADR-AppArch-25`).
//!
//! A stateless client: stdio MCP toward the agent, authenticated HTTPS toward
//! `meridian-backend`. It holds no copy of the Architecture Model and makes no AI calls.

mod backend;
mod change;
mod config;
mod install;
mod review_report;
mod session;
mod store;
mod setup;
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
    /// Add Meridian to the MCP configuration of the coding agents found on this machine (Claude Code, Claude Desktop,
    /// Cursor, VS Code), pointing at this program. Only the `meridian` entry is changed; the original file is saved once.
    Install {
        /// Set up only this agent: claude-code, claude-desktop, cursor or vscode.
        #[arg(long)]
        agent: Option<String>,
        /// Say what would change and change nothing.
        #[arg(long)]
        dry_run: bool,
        /// Take Meridian out of the agents' configuration instead.
        #[arg(long)]
        remove: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve().await,
        Command::Install { agent, dry_run, remove } => {
            let command = std::env::current_exe()?.canonicalize()?;
            let reports = install::run(&install::Env::current()?, &command, agent.as_deref(), remove, dry_run)?;
            print!("{}", install::describe(&reports, remove, dry_run));
            Ok(())
        }
    }
}

async fn serve() -> Result<()> {
    let config = config::Config::from_env()?;
    let store = store::Store::open(&config.api_url);
    let backend = backend::Backend::new(config)?;
    let service = tools::MeridianTools::new(backend, store).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
