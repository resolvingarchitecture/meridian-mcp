// Orchestrator
use crate::models::{
    AddContextRequest, ArchitectureContext, ArchitectureReviewReadiness,
    BitcoinFundingStatusRequest, CachedArchitectureReviewRequest, ChangedFile, ContentType,
    CreateAccountRequest, DocumentInput, DocumentTypeHint,
    RequestApiKeyRequest, RequestBitcoinFundingRequest, ReviewOptions,
    ScanProjectRequest,
};
use crate::scanner::documents;
use anyhow::{Context, Result};
use rmcp::{handler::server::tool::Parameters, model::ServerInfo, tool, ServerHandler};
use serde_json::json;
use std::path::Path;
use std::process::Command;
use tracing::info;
use uuid::Uuid;

mod agent;
mod cache;
mod config;
mod models;
mod scanner;

// ── MCP Server ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct MeridianServer;

#[tool(tool_box)]
impl MeridianServer {
    #[tool(
        description = "Create a Meridian account using the backend API. After this succeeds, call request_api_key once to generate and save an API key for MCP review tools."
    )]
    async fn create_account(&self, Parameters(req): Parameters<CreateAccountRequest>) -> String {
        info!("create_account");

        match agent::create_account(req).await {
            Ok(_) => {
                info!("create_account: complete");
                json!({
                    "status": "ok",
                    "message": "account created",
                    "nextStep": "Call request_api_key once with the same username and password to generate and save an API key."
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("create_account failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Login internally with username/password, request a one-time Meridian API key from the backend, and save it to local Meridian config. This should only be called again if the original API key becomes inactive."
    )]
    async fn request_api_key(&self, Parameters(req): Parameters<RequestApiKeyRequest>) -> String {
        info!("request_api_key");

        match agent::request_and_save_api_key(req).await {
            Ok(raw_api_key) => {
                info!("request_api_key: complete");
                json!({
                    "status": "ok",
                    "message": "API key generated and saved",
                    "apiKey": raw_api_key,
                    "configFile": config::config_file_display_path(),
                    "warning": "This raw API key is shown once. Store it securely if you need a separate copy. Do not call request_api_key again unless this key becomes inactive."
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("request_api_key failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Create a Bitcoin funding request for the authenticated Meridian account. Returns a fresh Bitcoin receive address, sats amount, USD estimate, exchange rate, expiration time, and status. The user should send exactly amountSats to the returned address before expiresAt."
    )]
    async fn request_bitcoin_funding(
        &self,
        Parameters(req): Parameters<RequestBitcoinFundingRequest>,
    ) -> String {
        info!("request_bitcoin_funding: amount_sats={}", req.amount_sats);

        match agent::request_bitcoin_funding(req.amount_sats).await {
            Ok(payment_request) => {
                info!(
                    "request_bitcoin_funding: complete — address={}",
                    payment_request.address
                );
                json!({
                    "status": "ok",
                    "message": "Bitcoin funding request created",
                    "payment": payment_request,
                    "instructions": [
                        "Send exactly the requested amountSats to the returned Bitcoin address before expiresAt.",
                        "After broadcasting the transaction, call bitcoin_funding_status with the returned address to check confirmation status.",
                        "Sats are credited after the backend confirms the Bitcoin payment according to its confirmation policy."
                    ]
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("request_bitcoin_funding failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Check the status of a Bitcoin funding request by receive address. Use the address returned from request_bitcoin_funding."
    )]
    async fn bitcoin_funding_status(
        &self,
        Parameters(req): Parameters<BitcoinFundingStatusRequest>,
    ) -> String {
        info!("bitcoin_funding_status: address={}", req.address);

        match agent::bitcoin_funding_status(&req.address).await {
            Ok(status) => {
                info!(
                    "bitcoin_funding_status: complete — status={}",
                    status.status
                );
                json!({
                    "status": "ok",
                    "payment": status
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("bitcoin_funding_status failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(
        description = "Scan a Meridian project directory and populate the cached architecture review request with discovered documents. \
        Call this once when opening a project. The request template is cached automatically."
    )]
    async fn scan_project(&self, Parameters(req): Parameters<ScanProjectRequest>) -> String {
        let root_dir = req.root_dir;
        info!("scan_project: {}", root_dir);
        let path = std::path::Path::new(&root_dir);

        if !path.exists() {
            return json!({ "error": format!("directory not found: {root_dir}") }).to_string();
        }

        match scan_into_cached_request(&root_dir) {
            Ok(cached) => {
                info!(
                    "scan_project: complete — documents={}",
                    cached.request.documents.len()
                );
                json!({
                    "status": "ok",
                    "context_id": cached.request.context_id,
                    "document_count": cached.request.documents.len(),
                    "documents": cached.request.documents
                })
                .to_string()
            }
            Err(e) => {
                tracing::error!("scan_project failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(description = "Run a review of the documents discovered in the scanned project.")]
    async fn run_review(&self) -> String {
        info!("run_review");

        let cached = match load_or_scan_request() {
            Ok(cached) => cached,
            Err(e) => {
                return json!({ "error": e.to_string() }).to_string();
            }
        };

        let request = cached.request_for_review(ReviewOptions::create(), None);

        match agent::run_review(&request).await {
            Ok(review_response) => {
                info!("run_review: complete");
                review_response.to_string()
            }
            Err(e) => {
                tracing::error!("run_full_review failed: {}", e);
                json!({ "error": e.to_string() }).to_string()
            }
        }
    }

    #[tool(description = "Clear the cached architecture model for a project. \
        Use this after major refactors to force a fresh scan.")]
    async fn invalidate_cache(&self) -> String {
        match cache::invalidate() {
            Ok(_) => json!({ "status": "cache cleared" }).to_string(),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }
}

fn load_cached_request() -> Result<CachedArchitectureReviewRequest> {
    match cache::get()? {
        Some(cached) => Ok(cached),
        None => {
            anyhow::bail!(
                "no cached architecture review request found. Run scan_project(root_dir) first."
            );
        }
    }
}

fn load_or_scan_request() -> Result<CachedArchitectureReviewRequest> {
    match cache::get()? {
        Some(cached) => Ok(cached),
        None => {
            let root = std::env::current_dir().context("failed to determine current directory")?;
            scan_into_cached_request(
                root.to_str()
                    .context("current directory path contains invalid UTF-8")?,
            )
        }
    }
}

fn collect_git_working_tree_diff(root_dir: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff", "--binary"])
        .current_dir(root_dir)
        .output()
        .with_context(|| format!("failed to run git diff in: {root_dir}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed in {root_dir}: {}", stderr.trim());
    }

    String::from_utf8(output.stdout).context("git diff output was not valid UTF-8")
}

fn scan_into_cached_request(root_dir: &str) -> Result<CachedArchitectureReviewRequest> {
    let path = std::path::Path::new(root_dir);

    if !path.exists() {
        anyhow::bail!("directory not found: {root_dir}");
    }

    if !path.is_dir() {
        anyhow::bail!("not a directory: {root_dir}");
    }

    let documents = scanner::scan(path)
        .with_context(|| format!("failed to scan architecture documents for: {root_dir}"))?;

    let mut cached =
        cache::get()?.unwrap_or_else(|| CachedArchitectureReviewRequest::new(Uuid::new_v4()));

    cached.upsert_documents(documents);
    synchronize_documents_into_model(&mut cached);

    cache::set(&cached).with_context(|| format!("failed to cache architecture review request"))?;

    Ok(cached)
}

fn synchronize_documents_into_model(cached: &mut CachedArchitectureReviewRequest) {
    let model = &mut cached.request.architecture_model;

    for document in &cached.request.documents {
        let Some(filename) = document.filename.as_deref() else {
            continue;
        };

        if matches!(
            document.type_hint,
            DocumentTypeHint::ArchitectureDecisionRecord
        ) && !model
            .global_observations
            .adrs
            .contains(&filename.to_string())
        {
            model.global_observations.adrs.push(filename.to_string());
        }

        if model
            .evidence
            .iter()
            .any(|evidence| evidence.path.as_deref() == Some(filename))
        {
            continue;
        }

        model.evidence.push(crate::models::ArchitectureEvidence {
            evidence_id: document.id.clone(),
            source_type: format!("{:?}", document.type_hint),
            path: Some(filename.to_string()),
            description: document
                .stated_scope
                .clone()
                .unwrap_or_else(|| document.title.clone()),
            scanned_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_secs()),
        });
    }
}

fn scan_component_into_cached_model(root_dir: &str) -> Result<crate::models::ArchitectureModel> {
    let cached = scan_into_cached_request(root_dir)?;
    Ok(cached.request.architecture_model)
}

fn add_context_to_cached_model(
    mut context: ArchitectureContext,
) -> Result<CachedArchitectureReviewRequest> {
    let mut cached =
        cache::get()?.unwrap_or_else(|| CachedArchitectureReviewRequest::new(Uuid::new_v4()));

    let context_id = context.context_id.unwrap_or(cached.request.context_id);
    context.context_id = Some(context_id);

    cached.request.context_id = context_id;
    cached.request.architecture_model.context_id = Some(context_id);
    cached.request.architecture_model.context = context;

    cache::set(&cached).with_context(|| format!("failed to cache architecture context"))?;

    Ok(cached)
}

fn persist_readiness_context(readiness: &ArchitectureReviewReadiness) -> Result<()> {
    let Some(context) = readiness.architecture_context.clone() else {
        return Ok(());
    };

    let mut cached = load_cached_request()?;

    if let Some(context_id) = context.context_id {
        cached.request.context_id = context_id;
        cached.request.architecture_model.context_id = Some(context_id);
    }

    cached.request.architecture_model.context = context;

    cache::set(&cached).with_context(|| {
        format!("failed to cache architecture context returned by review readiness")
    })?;

    Ok(())
}

fn reviewed_change_set_document(
    changes: String,
    change_summary: Option<String>,
    changed_files: Option<Vec<ChangedFile>>,
) -> DocumentInput {
    let changed_files_note = changed_files
        .as_ref()
        .filter(|files| !files.is_empty())
        .map(|files| {
            let files = files
                .iter()
                .map(|file| format!("{} ({:?})", file.path, file.change_type))
                .collect::<Vec<_>>()
                .join(", ");

            format!(" Changed files: {files}.")
        })
        .unwrap_or_default();

    let stated_scope = match change_summary {
        Some(summary) if !summary.trim().is_empty() => Some(format!(
            "Submitted change set for intermediate architecture review. Summary: {}.{}",
            summary.trim(),
            changed_files_note
        )),
        _ => Some(format!(
            "Submitted change set for intermediate architecture review.{}",
            changed_files_note
        )),
    };

    documents::new_document_input(
        Path::new("submitted-change-set"),
        "submitted-change-set".to_string(),
        DocumentTypeHint::Codebase,
        stated_scope,
        ContentType::Code,
        "text/x-diff",
        changes,
    )
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
        Some("components") => match args.get(2).map(String::as_str) {
            Some("list") => cli_components_list(),
            _ => {
                anyhow::bail!("usage: meridian components list");
            }
        },
        Some("account") => match args.get(2).map(String::as_str) {
            Some("create") => {
                let username = args.get(3).context(
                    "usage: meridian account create <username> <password> [email] [phone]",
                )?;
                let password = args.get(4).context(
                    "usage: meridian account create <username> <password> [email] [phone]",
                )?;
                let email = args.get(5).cloned().unwrap_or_default();
                let phone = args.get(6).cloned().unwrap_or_default();

                cli_create_account(username, password, &email, &phone).await
            }
            Some("request-api-key") => {
                let username = args
                    .get(3)
                    .context("usage: meridian account request-api-key <username> <password>")?;
                let password = args
                    .get(4)
                    .context("usage: meridian account request-api-key <username> <password>")?;

                cli_request_api_key(username, password).await
            }
            _ => {
                anyhow::bail!(
                    "usage: meridian account create <username> <password> [email] [phone]\n       meridian account request-api-key <username> <password>"
                );
            }
        },
        Some("fund") => match args.get(2).map(String::as_str) {
            Some("bitcoin") => match args.get(3).map(String::as_str) {
                Some("status") => {
                    let address = args
                        .get(4)
                        .context("usage: meridian fund bitcoin status <address>")?;
                    cli_bitcoin_funding_status(address).await
                }
                Some(amount_sats) => cli_request_bitcoin_funding(amount_sats).await,
                None => {
                    anyhow::bail!(
                        "usage: meridian fund bitcoin <amount_sats>\n       meridian fund bitcoin status <address>"
                    );
                }
            },
            _ => {
                anyhow::bail!(
                    "usage: meridian fund bitcoin <amount_sats>\n       meridian fund bitcoin status <address>"
                );
            }
        },
        Some("review") => cli_review().await,
        Some("cache") => match args.get(2).map(String::as_str) {
            Some("clear") => cli_cache_clear(),
            _ => {
                anyhow::bail!("usage: meridian cache clear");
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
                anyhow::bail!("usage: meridian context <template|add [json_file]>");
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
            print_version();
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
        entry["model"]["globalObservations"]["adrs"]
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

fn cli_components_list() -> Result<()> {
    let cached = load_cached_request()?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "context_id": cached.request.context_id,
            "components": cached
                .request
                .architecture_model
                .components
                .iter()
                .map(|component| {
                    json!({
                        "id": component.component_id,
                        "name": component.name
                    })
                })
                .collect::<Vec<_>>()
        }))?
    );

    Ok(())
}

async fn cli_create_account(
    username: &str,
    password: &str,
    email: &str,
    phone: &str,
) -> Result<()> {
    agent::create_account(CreateAccountRequest {
        username: username.to_string(),
        password: password.to_string(),
        email: email.to_string(),
        phone: phone.to_string(),
    })
    .await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "account created",
            "next_steps": [
                "Run: meridian-mcp account request-api-key <username> <password>",
                "After the API key is generated and saved, authenticated CLI commands can use it automatically."
            ]
        }))?
    );

    Ok(())
}

async fn cli_request_api_key(username: &str, password: &str) -> Result<()> {
    let raw_api_key = agent::request_and_save_api_key(RequestApiKeyRequest {
        username: username.to_string(),
        password: password.to_string(),
    })
    .await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "API key generated and saved",
            "apiKey": raw_api_key,
            "configFile": config::config_file_display_path(),
            "warning": "This raw API key is shown once. Store it securely if you need a separate copy. Do not run request-api-key again unless this key becomes inactive.",
            "next_steps": [
                "Run: meridian-mcp login",
                "Optionally fund the account: meridian-mcp fund bitcoin <amount_sats>",
                "Then run review commands normally."
            ]
        }))?
    );

    Ok(())
}

async fn cli_request_bitcoin_funding(amount_sats: &str) -> Result<()> {
    let amount_sats = amount_sats
        .parse::<u64>()
        .with_context(|| format!("amount_sats must be a positive integer: {amount_sats}"))?;

    if amount_sats == 0 {
        anyhow::bail!("amount_sats must be greater than zero");
    }

    let payment_request = agent::request_bitcoin_funding(amount_sats).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "message": "Bitcoin funding request created",
            "payment": payment_request,
            "instructions": [
                "Send exactly the requested amountSats to the returned Bitcoin address before expiresAt.",
                "After broadcasting the transaction, run: meridian-mcp fund bitcoin status <address>",
                "Sats are credited after the backend confirms the Bitcoin payment according to its confirmation policy."
            ]
        }))?
    );

    Ok(())
}

async fn cli_bitcoin_funding_status(address: &str) -> Result<()> {
    let status = agent::bitcoin_funding_status(address).await?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "payment": status
        }))?
    );

    Ok(())
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
                "meridian-mcp review readiness",
                "meridian-mcp review full"
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
                "meridian-mcp review readiness",
                "meridian-mcp review full"
            ],
            "after_successful_full_review": [
                "meridian-mcp review intermediate <changes_file>"
            ],
            "multi_source_note": "Meridian preserves source-root identity locally. The backend determines whether these roots belong to the same architecture context or review baseline."
        })
    }
}

async fn cli_review() -> Result<()> {
    let cached = load_or_scan_request()?;

    let request = cached.request_for_review(ReviewOptions::create(), None);

    let review_response = agent::run_review(&request).await?;

    println!("{}", serde_json::to_string_pretty(&review_response)?);

    Ok(())
}

fn cli_review_guidance() -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "guidance",
            "message": "Meridian review is a staged workflow. Scan architecture-significant sources first, add context if ADRs are missing, then run a full review. Intermediate review is only valid after a successful full review has established a backend baseline.",
            "workflow": [
                {
                    "step": 1,
                    "command": "meridian-mcp scan [root_dir]",
                    "purpose": "Scan a directory containing architecture-significant artifacts, ADRs, architecture docs, code structure, infrastructure definitions, or related design material."
                },
                {
                    "step": 2,
                    "command": "meridian-mcp context template > meridian-context.json",
                    "purpose": "Use this if the scan finds no ADRs or if the backend needs more architecture context."
                },
                {
                    "step": 3,
                    "command": "meridian-mcp context add meridian-context.json",
                    "purpose": "Send stakeholders, concerns, agreed decisions, constraints, risks, standards, and non-functional requirements to the backend as architecture context."
                },
                {
                    "step": 4,
                    "command": format!("meridian-mcp review"),
                    "purpose": "Attempt a full review. A successful full review establishes the backend baseline for later intermediate reviews."
                },
                {
                    "step": 6,
                    "command": format!("meridian-mcp review"),
                    "purpose": "Subsequent review calls are intermediate reviews when changes are small."
                }
            ],
            "decision_authority": "Meridian recommends. The customer organization decides."
        }))?
    );

    Ok(())
}

fn cli_cache_clear() -> Result<()> {
    cache::invalidate().with_context(|| format!("failed to clear cache"))?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "cache cleared"
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

    let context: ArchitectureContext = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse context JSON: {file_path}"))?;

    let cached = add_context_to_cached_model(context)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "ok",
            "context_id": cached.request.context_id,
            "message": "architecture context added to cached ArchitectureModel",
            "next_steps": [
                "meridian-mcp review readiness",
                "meridian-mcp review full",
                "After a successful full review, use: meridian-mcp review intermediate <changes_file>"
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
            "version": version_info(),
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

fn version_info() -> serde_json::Value {
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "git_commit": option_env!("MERIDIAN_GIT_HASH").unwrap_or("unknown"),
        "git_dirty": option_env!("MERIDIAN_GIT_DIRTY").unwrap_or("unknown"),
        "built_at": option_env!("MERIDIAN_BUILD_TIMESTAMP").unwrap_or("unknown"),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" }
    })
}

fn print_version() {
    println!(
        "meridian {}\ncommit: {}\ndirty: {}\nbuilt: {}\nprofile: {}",
        env!("CARGO_PKG_VERSION"),
        option_env!("MERIDIAN_GIT_HASH").unwrap_or("unknown"),
        option_env!("MERIDIAN_GIT_DIRTY").unwrap_or("unknown"),
        option_env!("MERIDIAN_BUILD_TIMESTAMP").unwrap_or("unknown"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
}

fn print_help() {
    println!(
        r#"Meridian architecture assistant MCP server and CLI

Usage:
  meridian-mcp mcp
  meridian-mcp scan [root_dir...]
  meridian-mcp components list
  meridian-mcp account create <username> <password> [email] [phone]
  meridian-mcp account request-api-key <username> <password>
  meridian-mcp context template
  meridian-mcp context add <json_file>
  meridian-mcp fund bitcoin <amount_sats>
  meridian-mcp fund bitcoin status <address>
  meridian-mcp review
  meridian-mcp cache clear
  meridian-mcp config set api-key <key>
  meridian-mcp config set backend-url <url>
  meridian-mcp test backend
  meridian-mcp login
  meridian-mcp logout
  meridian-mcp doctor
  meridian-mcp version
  meridian-mcp help

MCP account setup tools:
  create_account
      Create a Meridian account using username, password, email (optional), and phone (optional).

  request_api_key
      Login internally with username and password, request a one-time Meridian API key,
      and save it to local Meridian config for review tools.

      Call this once after account creation. Only call it again if the original
      API key becomes inactive.

CLI account setup commands:
  meridian-mcp account create <username> <password> [email] [phone]
      Create a Meridian account using the backend API.
      Username and password are required.
      Email and phone are optional.

  meridian-mcp account request-api-key <username> <password>
      Login with username/password, request a one-time Meridian API key,
      and save it to local Meridian config.

      Call this once after account creation. Only call it again if the original
      API key becomes inactive.

MCP account funding tools:
  request_bitcoin_funding
      Create a Bitcoin payment request for the authenticated account.
      Input: amountSats.
      Returns a fresh Bitcoin receive address, amount, USD estimate,
      exchange rate, expiration time, and status.

  bitcoin_funding_status
      Check the status of a Bitcoin funding request by receive address.

CLI account funding commands:
  meridian-mcp fund bitcoin <amount_sats>
      Create a Bitcoin payment request for the authenticated account.
      Returns a fresh Bitcoin receive address, amount, USD estimate,
      exchange rate, expiration time, and status.

  meridian-mcp fund bitcoin status <address>
      Check the status of a Bitcoin funding request by receive address.

Recommended Setup for MCP agents:
    1. Start the MCP server:
        meridian-mcp mcp

    2. If the user does not already have a Meridian account, call:
        create_account

    3. Then call:
        request_api_key

       This generates the user's API key and saves it locally. The key starts
       with m_live_ and is shown once.

    4. After request_api_key succeeds, review tools will authenticate automatically.

Recommended Setup for CLI users:
    1. If you do not already have a Meridian account, create one:
        meridian-mcp account create <username> <password> [email] [phone]

    2. Request and save a local API key:
        meridian-mcp account request-api-key <username> <password>

       This generates the user's API key and saves it locally. The key starts
       with m_live_ and is shown once.

    3. If you already have an API key, set it in local config instead:
        meridian-mcp config set api-key m_live_restofkeyhere

    4. Test backend access:
        meridian-mcp test backend
        Returns: {{
                   "status": "UP|DOWN",
                   "timestamp": "date-time-here"
                 }}

    5. Test backend authentication:
        meridian-mcp login
        Returns: {{
                   "sessionId": "ID-here",
                   "expiresAt": 30-minutes-ahead-in-epoch-time-here",
                   "status": 200,
                   "message": "Login successful"
                 }}

    6. Fund the account with Bitcoin:
        meridian-mcp fund bitcoin 100000

        Send the Bitcoin to the address returned.

       Then check payment status:
        meridian-mcp fund bitcoin status bc1...

Recommended review workflow:
    1. Scan one or more architecture-significant source roots:
        meridian-mcp scan [root_dir...]

        Examples:
        meridian-mcp scan .
        meridian-mcp scan ../frontend ../backend ../infra ../architecture-docs

     Each root is scanned and cached into a single ArchitectureModel. Architecture can span
     multiple projects, repositories, infrastructure roots, API contracts,
     ADR folders, and architecture-document locations.

    2. If no ADRs or architecture records are present in one or more sources:
       meridian-mcp context template > meridian-context.json
       Edit meridian-context.json with stakeholders, stakeholder concerns,
       agreed decisions, constraints, risks, standards, scope notes, and
       non-functional requirements.

    3. Add context to the Meridian backend:
       meridian-mcp context add meridian-context.json

    4. Prepare and run a full review:
       meridian-mcp review readiness
       meridian-mcp review full

    5. After a successful full review establishes a backend baseline:
       git diff > changes.diff
       meridian-mcp review intermediate changes.diff

Notes:
    meridian-mcp review
       Prints workflow guidance.

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
        "meridian starting in MCP mode (v{}, commit={}, built_at={})",
        env!("CARGO_PKG_VERSION"),
        option_env!("MERIDIAN_GIT_HASH").unwrap_or("unknown"),
        option_env!("MERIDIAN_BUILD_TIMESTAMP").unwrap_or("unknown")
    );

    if crate::config::api_key().is_err() {
        eprintln!(
            "ERROR: MERIDIAN_API_KEY environment variable not set and no local API key configured."
        );
        eprintln!("Account setup tools are still available.");
        eprintln!(
            "To enable review tools, call create_account if needed, then request_api_key once."
        );
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
