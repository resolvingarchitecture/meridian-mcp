use anyhow::{Context, Result};
use rmcp::{handler::server::tool::Parameters, model::ServerInfo, schemars, tool, ServerHandler};
use serde::Deserialize;
use serde_json::json;
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
struct AddContextRequest {
    #[schemars(with = "Option<String>")]
    context_id: Option<uuid::Uuid>,
    organization_context: Option<serde_json::Value>,
    business_goals: Option<Vec<String>>,
    stakeholders: Option<Vec<serde_json::Value>>,
    decisions: Option<Vec<serde_json::Value>>,
    constraints: Option<Vec<String>>,
    risks: Option<Vec<String>>,
    standards: Option<Vec<String>>,
    scope_notes: Option<Vec<String>>,
    freeform_notes: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ScanProjectRequest {
    root_dir: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FullReviewPromptRequest {
    root_dir: String,
    file_path: String,
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FullReviewRequest {
    root_dir: String,
    file_path: String,
    content: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct IntermediateReviewRequest {
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
    #[tool(
        description = "Add persistent architecture context to the Meridian backend. \
            Returns a context_id that can be included in subsequent reviews."
    )]
    async fn add_context(&self, Parameters(req): Parameters<AddContextRequest>) -> String {
        info!("add_context");

        let context = agent::ArchitectureContext {
            context_id: req.context_id,
            organization_context: req.organization_context,
            business_goals: req.business_goals,
            stakeholders: req.stakeholders,
            decisions: req.decisions,
            constraints: req.constraints,
            risks: req.risks,
            standards: req.standards,
            scope_notes: req.scope_notes,
            freeform_notes: req.freeform_notes,
        };

        match agent::add_context(context).await {
            Ok(response) => {
                info!("add_context: complete — context_id={}", response.context_id);
                json!({
                    "context_id": response.context_id,
                    "context_percent_used": response.context_percent_used,
                    "message": response.message
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("add_context failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Scan a Meridian project directory and build its architecture model. \
        Call this once when opening a project. The model is cached automatically."
    )]
    async fn scan_project(&self, Parameters(req): Parameters<ScanProjectRequest>) -> String {
        let root_dir = req.root_dir;
        info!("scan_project: {}", root_dir);
        let path = std::path::Path::new(&root_dir);

        if !path.exists() {
            return json!({ "error": format!("directory not found: {root_dir}") }).to_string();
        }

        match scan_component_into_cached_model(&root_dir) {
            Ok(model) => {
                info!(
                    "scan_project: complete — components={}, style={}, layers={:?}",
                    model.components.len(),
                    model.style_summary(),
                    model.layer_order_summary()
                );
                json!({ "status": "ok", "model": model }).to_string()
            }
            Err(e) => {
                tracing::error!("scan_project failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Stage 1 of the review workflow. Build the full-review prompt. \
            This must be called before run_full_review."
    )]
    async fn build_full_review_prompt(
        &self,
        Parameters(req): Parameters<FullReviewPromptRequest>,
    ) -> String {
        let root_dir = req.root_dir;
        let file_path = req.file_path;
        let content = req.content;

        info!("build_full_review_prompt: {}", file_path);

        let model = match load_or_scan_model(&root_dir) {
            Ok(model) => model,
            Err(e) => {
                return json!({ "error": e.to_string() }).to_string();
            }
        };

        match agent::build_full_review_prompt(&model, &file_path, &content).await {
            Ok(prompt) => {
                info!("build_full_review_prompt: complete for {}", file_path);
                json!({
                    "context_id": prompt.context_id,
                    "status": prompt.status,
                    "question": prompt.question,
                    "domain_estimates": prompt.domain_estimates,
                    "sats_available": prompt.sats_available,
                    "total_estimated_price": prompt.total_estimated_price,
                    "requires_user_selection": prompt.requires_user_selection,
                    "present_estimated_price": prompt.present_estimated_price(),
                    "present_domains_exceed_available_balance": prompt.present_domains_exceed_available_balance(),
                    "selection_guidance": prompt.selection_guidance(),
                    "insufficient_balance_reminder": if prompt.present_domains_exceed_available_balance() {
                        Some("The currently present domains exceed sats_available. Ask the user to choose fewer domains or add more funds before continuing.")
                    } else {
                        None
                    }
                }).to_string()
            }
            Err(e) => {
                tracing::error!("build_full_review_prompt failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(description = "Stage 2 of the review workflow. Run the full review. \
        This must be called after build_full_review_prompt and before run_intermediate_review.")]
    async fn run_full_review(&self, Parameters(req): Parameters<FullReviewRequest>) -> String {
        let root_dir = req.root_dir;
        let file_path = req.file_path;
        let content = req.content;

        info!("run_full_review: {}", file_path);

        let model = match load_or_scan_model(&root_dir) {
            Ok(model) => model,
            Err(e) => {
                return json!({ "error": e.to_string() }).to_string();
            }
        };

        match agent::run_full_review(&model, &file_path, &content).await {
            Ok(findings) => {
                info!(
                    "run_full_review: {} finding(s) for {}",
                    findings.len(),
                    file_path
                );
                json!({ "findings": findings }).to_string()
            }
            Err(e) => {
                tracing::error!("run_full_review failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Stage 3 of the review workflow. Run an intermediate review for a file change. \
        This must be called only after run_full_review has completed."
    )]
    async fn run_intermediate_review(
        &self,
        Parameters(req): Parameters<IntermediateReviewRequest>,
    ) -> String {
        let root_dir = req.root_dir;
        let file_path = req.file_path;
        let content = req.content;

        info!("run_intermediate_review: {}", file_path);

        let model = match load_or_scan_model(&root_dir) {
            Ok(model) => model,
            Err(e) => {
                return json!({ "error": e.to_string() }).to_string();
            }
        };

        match agent::run_intermediate_review(&model, &file_path, &content).await {
            Ok(findings) => {
                info!(
                    "run_intermediate_review: {} finding(s) for {}",
                    findings.len(),
                    file_path
                );
                json!({ "findings": findings }).to_string()
            }
            Err(e) => {
                tracing::error!("run_intermediate_review failed: {}", e);
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

fn load_or_scan_model(root_dir: &str) -> Result<scanner::ArchitectureModel> {
    if let Some(model) = cache::get(root_dir)? {
        return Ok(model);
    }

    let path = std::path::Path::new(root_dir);
    let component = scanner::scan(path)
        .with_context(|| format!("could not scan architecture component for: {root_dir}"))?;

    let model = scanner::ArchitectureModel::from_component(component);

    cache::set(root_dir, &model)
        .with_context(|| format!("failed to cache architecture model for: {root_dir}"))?;

    Ok(model)
}

fn scan_component_into_cached_model(root_dir: &str) -> Result<scanner::ArchitectureModel> {
    let path = std::path::Path::new(root_dir);
    let component = scanner::scan(path)
        .with_context(|| format!("failed to scan architecture component for: {root_dir}"))?;

    let mut model = cache::get(root_dir)?
        .unwrap_or_else(|| scanner::ArchitectureModel::new(Some(root_dir.to_string())));

    model.add_component(component);

    cache::set(root_dir, &model)
        .with_context(|| format!("failed to cache architecture model for: {root_dir}"))?;

    Ok(model)
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
            let roots: Vec<String> = if args.len() > 2 {
                args[2..].to_vec()
            } else {
                vec![".".to_string()]
            };
            cli_scan(&roots)
        }
        Some("review") => match args.get(2).map(String::as_str) {
            Some("prompt") => {
                let file_path = args
                    .get(3)
                    .context("usage: meridian review prompt <file_path>")?;
                cli_review_prompt(file_path).await
            }
            Some("full") => {
                let file_path = args
                    .get(3)
                    .context("usage: meridian review full <file_path>")?;
                cli_review_full(file_path).await
            }
            Some("intermediate") => {
                let file_path = args
                    .get(3)
                    .context("usage: meridian review intermediate <file_path>")?;
                cli_review_intermediate(file_path).await
            }
            Some(file_path) => cli_review_guidance(file_path),
            None => {
                anyhow::bail!(
                    "usage: meridian review <file_path>\n       meridian review <prompt|full|intermediate> <file_path>"
                );
            }
        },
        Some("cache") => match args.get(2).map(String::as_str) {
            Some("clear") => {
                let root = args.get(3).map(String::as_str).unwrap_or(".");
                cli_cache_clear(root)
            }
            _ => {
                anyhow::bail!("usage: meridian cache clear [root_dir]");
            }
        },
        Some("context") => match args.get(2).map(String::as_str) {
            Some("template") => cli_context_template(),
            Some("add") => {
                let file_path = args
                    .get(3)
                    .context("usage: meridian context add <json_file>")?;
                cli_context_add(file_path).await
            }
            _ => {
                anyhow::bail!("usage: meridian context <template|add> [json_file]");
            }
        },
        Some("test") => match args.get(2).map(String::as_str) {
            Some("backend") => cli_test_backend().await,
            _ => {
                anyhow::bail!("usage: meridian test backend");
            }
        },
        Some("login") => cli_login().await,
        Some("logout") => cli_logout().await,
        Some("config") => match (
            args.get(2).map(String::as_str),
            args.get(3).map(String::as_str),
        ) {
            (Some("set"), Some("api-key")) => {
                let key = args
                    .get(4)
                    .context("usage: meridian config set api-key <key>")?;
                cli_config_set_api_key(key)
            }
            (Some("set"), Some("backend-url")) => {
                let backend_url = args
                    .get(4)
                    .context("usage: meridian config set backend-url <url>")?;
                cli_config_set_backend_url(backend_url)
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

fn cli_scan(roots: &[String]) -> Result<()> {
    let mut scanned = Vec::new();

    for root in roots {
        scanned.push(cli_scan_one(root)?);
    }

    let any_missing_adrs = scanned.iter().any(|entry| {
        entry["model"]["global_observations"]["adrs"]
            .as_array()
            .is_some_and(|adrs| adrs.is_empty())
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "source_count": scanned.len(),
            "sources": scanned,
            "workflow_guidance": multi_source_scan_workflow_guidance(any_missing_adrs)
        }))?
    );

    Ok(())
}

fn cli_scan_one(root: &str) -> Result<serde_json::Value> {
    let path = Path::new(root);

    if !path.exists() {
        anyhow::bail!("directory not found: {root}");
    }

    if !path.is_dir() {
        anyhow::bail!("not a directory: {root}");
    }

    let model = scan_component_into_cached_model(root)
        .with_context(|| format!("failed to scan source root: {root}"))?;

    Ok(json!({
        "status": "ok",
        "root": root,
        "model": model
    }))
}

fn multi_source_scan_workflow_guidance(any_missing_adrs: bool) -> serde_json::Value {
    if any_missing_adrs {
        json!({
            "next_step": "add_context",
            "message": "One or more scanned source roots did not contain ADRs or architecture records. For a multi-source architecture review, add architecture context before attempting a full review.",
            "recommended_commands": [
                "meridian context template > meridian-context.json",
                "Edit meridian-context.json with stakeholders, concerns, agreed decisions, constraints, risks, standards, scope notes, and non-functional requirements.",
                "meridian context add meridian-context.json",
                "meridian review prompt <file_path>",
                "meridian review full <file_path>"
            ],
            "multi_source_note": "Each scanned root is cached independently. The backend remains responsible for deciding how roots relate to one architecture context, review scope, and full-review baseline.",
            "context_to_collect": [
                "architecture-significant source roots",
                "ADR and architecture-document roots",
                "infrastructure and deployment roots",
                "API contract or integration roots",
                "stakeholders and decision makers",
                "stakeholder concerns",
                "agreed architecture decisions",
                "non-functional requirements",
                "constraints",
                "risks",
                "standards",
                "scope notes"
            ]
        })
    } else {
        json!({
            "next_step": "full_review",
            "message": "Scanned source roots were cached independently. You can now prepare a full review with the backend.",
            "recommended_commands": [
                "meridian review prompt <file_path>",
                "meridian review full <file_path>"
            ],
            "after_successful_full_review": [
                "meridian review intermediate <file_path>"
            ],
            "multi_source_note": "Meridian preserves source-root identity locally. The backend determines whether these roots belong to the same architecture context or review baseline."
        })
    }
}

async fn cli_review_prompt(file_path: &str) -> Result<()> {
    let (model, content) = prepare_cli_review(file_path)?;

    let prompt = agent::build_full_review_prompt(&model, file_path, &content).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "context_id": prompt.context_id,
            "status": prompt.status,
            "question": prompt.question,
            "domain_estimates": prompt.domain_estimates,
            "sats_available": prompt.sats_available,
            "total_estimated_price": prompt.total_estimated_price,
            "requires_user_selection": prompt.requires_user_selection,
            "present_estimated_price": prompt.present_estimated_price(),
            "present_domains_exceed_available_balance": prompt.present_domains_exceed_available_balance(),
            "selection_guidance": prompt.selection_guidance(),
            "insufficient_balance_reminder": if prompt.present_domains_exceed_available_balance() {
                Some("The currently present domains exceed sats_available. Choose fewer domains or add more funds before continuing.")
            } else {
                None
            }
        }))?
    );

    Ok(())
}

async fn cli_review_full(file_path: &str) -> Result<()> {
    let (model, content) = prepare_cli_review(file_path)?;

    let findings = agent::run_full_review(&model, file_path, &content).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "findings": findings
        }))?
    );

    Ok(())
}

async fn cli_review_intermediate(file_path: &str) -> Result<()> {
    let (model, content) = prepare_cli_review(file_path)?;

    let findings = agent::run_intermediate_review(&model, file_path, &content).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "findings": findings
        }))?
    );

    Ok(())
}

fn cli_review_guidance(file_path: &str) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "guidance",
            "file_path": file_path,
            "message": "Meridian review is a staged workflow. Scan architecture-significant sources first, add context if ADRs are missing, then run a full review. Intermediate review is only valid after a successful full review has established a backend baseline.",
            "workflow": [
                {
                    "step": 1,
                    "command": "meridian scan [root_dir]",
                    "purpose": "Scan a directory containing architecture-significant artifacts, ADRs, architecture docs, code structure, infrastructure definitions, or related design material."
                },
                {
                    "step": 2,
                    "command": "meridian context template > meridian-context.json",
                    "purpose": "Use this if the scan finds no ADRs or if the backend needs more architecture context."
                },
                {
                    "step": 3,
                    "command": "meridian context add meridian-context.json",
                    "purpose": "Send stakeholders, concerns, agreed decisions, constraints, risks, standards, and non-functional requirements to the backend as architecture context."
                },
                {
                    "step": 4,
                    "command": format!("meridian review prompt {file_path}"),
                    "purpose": "Ask the backend for full-review preparation guidance, domain estimates, missing context, or selection prompts."
                },
                {
                    "step": 5,
                    "command": format!("meridian review full {file_path}"),
                    "purpose": "Attempt a full review. A successful full review establishes the backend baseline for later intermediate reviews."
                },
                {
                    "step": 6,
                    "command": format!("meridian review intermediate {file_path}"),
                    "purpose": "Use only after a successful full review baseline exists for the relevant architecture scope."
                }
            ],
            "decision_authority": "Meridian recommends. The customer organization decides."
        }))?
    );

    Ok(())
}

fn prepare_cli_review(file_path: &str) -> Result<(scanner::ArchitectureModel, String)> {
    let file = PathBuf::from(file_path);

    if !file.exists() {
        anyhow::bail!("file not found: {file_path}");
    }

    if !file.is_file() {
        anyhow::bail!("not a file: {file_path}");
    }

    let content = std::fs::read_to_string(&file)
        .with_context(|| format!("failed to read file: {file_path}"))?;

    let root = std::env::current_dir().context("failed to determine current directory")?;

    let root_str = root.to_string_lossy().to_string();

    let model = match cache::get(&root_str)? {
        Some(model) => model,
        None => {
            let component = scanner::scan(&root)
                .with_context(|| format!("failed to scan project: {}", root.display()))?;
            let model = scanner::ArchitectureModel::from_component(component);
            cache::set(&root_str, &model)?;
            model
        }
    };

    Ok((model, content))
}

fn cli_cache_clear(root: &str) -> Result<()> {
    cache::invalidate(root).with_context(|| format!("failed to clear cache for: {root}"))?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "cache cleared",
            "root": root
        }))?
    );

    Ok(())
}

fn cli_context_template() -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "organization_context": {
                "name": "",
                "domain": "",
                "system_or_product": "",
                "summary": ""
            },
            "business_goals": [
                ""
            ],
            "stakeholders": [
                {
                    "name": "",
                    "role": "",
                    "decision_authority": "",
                    "concerns": [
                        ""
                    ]
                }
            ],
            "decisions": [
                {
                    "title": "",
                    "status": "proposed|accepted|rejected|superseded",
                    "rationale": "",
                    "consequences": [
                        ""
                    ]
                }
            ],
            "constraints": [
                ""
            ],
            "risks": [
                ""
            ],
            "standards": [
                ""
            ],
            "scope_notes": [
                ""
            ],
            "freeform_notes": "Add any missing architecture context, non-functional requirements, quality attributes, assumptions, or open questions here."
        }))?
    );

    Ok(())
}

async fn cli_context_add(file_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("failed to read context file: {file_path}"))?;

    let context: agent::ArchitectureContext = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse context JSON: {file_path}"))?;

    let response = agent::add_context(context).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "context_id": response.context_id,
            "context_percent_used": response.context_percent_used,
            "message": response.message,
            "next_steps": [
                "meridian review prompt <file_path>",
                "meridian review full <file_path>",
                "After a successful full review, use: meridian review intermediate <file_path>"
            ]
        }))?
    );

    Ok(())
}

async fn cli_test_backend() -> Result<()> {
    let response = agent::test_backend_health().await?;

    println!("{}", serde_json::to_string_pretty(&response)?);

    Ok(())
}

async fn cli_login() -> Result<()> {
    let response = agent::test_login().await?;

    println!("{}", serde_json::to_string_pretty(&response)?);

    Ok(())
}

async fn cli_logout() -> Result<()> {
    agent::logout().await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "session logged out"
        }))?
    );

    Ok(())
}

fn cli_config_set_api_key(key: &str) -> Result<()> {
    if !key.starts_with("m_live_") {
        eprintln!("warning: Meridian API keys usually start with `m_live_`");
    }

    config::set_api_key(key)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "API key saved",
            "config_file": config::config_file_display_path()
        }))?
    );

    Ok(())
}

fn cli_config_set_backend_url(backend_url: &str) -> Result<()> {
    config::set_backend_url(backend_url)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "Backend URL saved",
            "backend_url": backend_url.trim_end_matches('/'),
            "config_file": config::config_file_display_path()
        }))?
    );

    Ok(())
}

fn cli_doctor() -> Result<()> {
    let env_api_key = std::env::var("MERIDIAN_API_KEY").ok();
    let configured_api_key = config::load()?.api_key;

    let api_key_status = if env_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty())
    {
        "set via MERIDIAN_API_KEY"
    } else if configured_api_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty())
    {
        "set in local config"
    } else {
        "missing"
    };

    let backend_url = config::backend_url()?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": if api_key_status == "missing" { "warning" } else { "ok" },
            "version": env!("CARGO_PKG_VERSION"),
            "api_key": api_key_status,
            "backend_url": backend_url,
            "config_file": config::config_file_display_path()
        }))?
    );

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
  meridian scan [root_dir...]
  meridian context template
  meridian context add <json_file>
  meridian review <file_path>
  meridian review prompt <file_path>
  meridian review full <file_path>
  meridian review intermediate <file_path>
  meridian cache clear [root_dir]
  meridian config set api-key <key>
  meridian config set backend-url <url>
  meridian test backend
  meridian login
  meridian logout
  meridian doctor
  meridian version
  meridian help

Recommended review workflow:
  1. Scan one or more architecture-significant source roots:
       meridian scan [root_dir...]

     Examples:
       meridian scan .
       meridian scan ../frontend ../backend ../infra ../architecture-docs

     Each root is scanned and cached independently. Architecture can span
     multiple projects, repositories, infrastructure roots, API contracts,
     ADR folders, and architecture-document locations.

  2. If no ADRs or architecture records are present in one or more sources:
       meridian context template > meridian-context.json
       Edit meridian-context.json with stakeholders, stakeholder concerns,
       agreed decisions, constraints, risks, standards, scope notes, and
       non-functional requirements.

  3. Add context to the Meridian backend:
       meridian context add meridian-context.json

  4. Prepare and run a full review:
       meridian review prompt <file_path>
       meridian review full <file_path>

  5. After a successful full review establishes a backend baseline:
       meridian review intermediate <file_path>

Notes:
  meridian review <file_path>
       Prints workflow guidance. It does not silently run intermediate review,
       because intermediate review requires a successful prior full review.

  Meridian preserves source-root identity locally. The backend decides how
  scanned roots relate to a larger architecture context, review scope, and
  full-review baseline.

  Meridian recommends. The customer organization decides.

Environment:
  MERIDIAN_API_KEY      API key, usually m_live_...
  MERIDIAN_BACKEND_URL  Backend URL
  MERIDIAN_LOG          Log level for stderr logs
"#
    );
}

async fn run_mcp_server() -> Result<()> {
    info!(
        "meridian starting in MCP mode (v{})",
        env!("CARGO_PKG_VERSION")
    );

    if crate::config::api_key().is_err() {
        eprintln!(
            "ERROR: MERIDIAN_API_KEY environment variable not set and no local API key configured."
        );
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
            std::env::var("MERIDIAN_LOG").unwrap_or_else(|_| "meridian=info".to_string()),
        )
        .init();

    run_cli(std::env::args().collect()).await
}
