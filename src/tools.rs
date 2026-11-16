//! The MCP tool surface (`ADR-AppArch-25`): `meridian_next`, `meridian_submit`,
//! `meridian_evaluate`, `meridian_model`. Each forwards to the backend.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Duration;

use crate::backend::{Backend, Reply};
use crate::session::Session;

pub struct MeridianTools {
    backend: Backend,
    session: Session,
}

#[derive(Deserialize, JsonSchema)]
pub struct NextParams {
    /// Meridian project (architecture context) ID.
    pub project: String,
    /// Structured observations about the project (paths, counts, short summaries, and the text of architecture documents such as ADRs).
    #[serde(default)]
    pub observations: Vec<Value>,
    /// Answers to earlier questions: `{ "questionId": "...", "value": ... }`.
    #[serde(default)]
    pub answers: Vec<Value>,
    /// The user's Architecture Passphrase. Ask the user if you do not have it.
    pub passphrase: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SubmitParams {
    pub project: String,
    /// Facts or answers to questions, each with its source and confidence.
    pub facts: Vec<Value>,
    /// Model version from a previous read; the write fails if the model has moved on.
    pub if_match: Option<u64>,
    pub passphrase: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct EvaluateParams {
    pub project: String,
    /// Service to run, for example `review`, `feasibility`, `tech_evaluation`.
    pub service: String,
    pub passphrase: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ModelParams {
    pub project: String,
    pub passphrase: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProjectsParams {
    /// `list` (default) shows the projects you can use; `create` makes a new one.
    pub action: Option<String>,
    /// Project name (required for `create`).
    pub name: Option<String>,
    /// Optional description for `create`.
    pub description: Option<String>,
    /// For `create`: the Architecture Passphrase to set for the new project. It cannot be recovered
    /// if lost, so ask the user to choose one (at least 8 characters).
    pub passphrase: Option<String>,
}

impl MeridianTools {
    pub fn new(backend: Backend) -> Self {
        Self { backend, session: Session::default() }
    }

    async fn forward(
        &self,
        method: Method,
        path: String,
        body: Option<Value>,
        passphrase: Option<String>,
        idempotent: bool,
        if_match: Option<u64>,
    ) -> Result<CallToolResult, McpError> {
        let held = self.session.resolve(passphrase);
        let text = match self
            .backend
            .call(method, &path, body, held.as_deref(), idempotent, if_match)
            .await
        {
            Reply::Ok(v) => return Ok(CallToolResult::success(vec![ContentBlock::text(v.to_string())])),
            Reply::PassphraseRequired | Reply::PassphraseIncorrect => {
                let wrong = held.is_some();
                self.session.clear();
                json!({
                    "error": if wrong { "passphrase_incorrect" } else { "passphrase_required" },
                    "instruction": "Ask the user for their Architecture Passphrase, then call \
                        this tool again with it in the `passphrase` field."
                })
                .to_string()
            }
            Reply::Failed(msg) => msg,
        };
        Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
    }
}

impl MeridianTools {
    fn finish(&self, reply: Reply, had_passphrase: bool) -> Result<CallToolResult, McpError> {
        let text = match reply {
            Reply::Ok(v) => return Ok(CallToolResult::success(vec![ContentBlock::text(v.to_string())])),
            Reply::PassphraseRequired | Reply::PassphraseIncorrect => {
                self.session.clear();
                json!({
                    "error": if had_passphrase { "passphrase_incorrect" } else { "passphrase_required" },
                    "instruction": "Ask the user for their Architecture Passphrase, then call \
                        this tool again with it in the `passphrase` field."
                })
                .to_string()
            }
            Reply::Failed(msg) => msg,
        };
        Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
    }
}


#[tool_router]
impl MeridianTools {
    #[tool(description = "List the Meridian projects you can use (action `list`, the default), or \
        create a new one (action `create`, with a name and a passphrase the user chooses). Call \
        this first to get the `project` ID the other tools need.")]
    async fn meridian_projects(&self, p: Parameters<ProjectsParams>) -> Result<CallToolResult, McpError> {
        let p = p.0;
        match p.action.as_deref().unwrap_or("list").to_ascii_lowercase().as_str() {
            "list" => self.forward(Method::GET, "/projects".into(), None, None, false, None).await,
            "create" => {
                let Some(name) = p.name.filter(|n| !n.trim().is_empty()) else {
                    return Ok(CallToolResult::error(vec![ContentBlock::text(
                        "`name` is required to create a project.")]));
                };
                self.forward(
                    Method::POST,
                    "/projects".into(),
                    Some(json!({ "name": name, "description": p.description })),
                    p.passphrase,
                    false,
                    None,
                )
                .await
            }
            other => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Unknown action `{other}`. Use `list` or `create`."))])),
        }
    }

    #[tool(description = "Submit observations (and answers to earlier questions) about the project \
        and get back the current phase, the gaps in the architecture, and the next questions. \
        Call repeatedly until `done` is true.")]
    async fn meridian_next(&self, p: Parameters<NextParams>) -> Result<CallToolResult, McpError> {
        let p = p.0;
        let idempotent = !p.answers.is_empty();
        self.forward(
            Method::POST,
            format!("/projects/{}/next", p.project),
            Some(json!({ "observations": p.observations, "answers": p.answers })),
            p.passphrase,
            idempotent,
            None,
        )
        .await
    }

    #[tool(description = "Submit facts (with source and confidence). They enter the Architecture \
        Model directly; there is no approval step.")]
    async fn meridian_submit(&self, p: Parameters<SubmitParams>) -> Result<CallToolResult, McpError> {
        let p = p.0;
        self.forward(
            Method::POST,
            format!("/projects/{}/submit", p.project),
            Some(json!({ "facts": p.facts })),
            p.passphrase,
            true,
            p.if_match,
        )
        .await
    }

    #[tool(description = "Run an entitled Meridian service (review, feasibility, and so on) \
        against the project. Waits for the result.")]
    async fn meridian_evaluate(
        &self,
        p: Parameters<EvaluateParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = p.0;
        let held = self.session.resolve(p.passphrase.clone());
        let path = format!("/projects/{}/evaluate", p.project);
        let body = json!({ "service": p.service });
        let first = self
            .backend
            .call(Method::POST, &path, Some(body), held.as_deref(), true, None)
            .await;
        // A 202 carries an evaluationId; poll until the result is ready.
        if let Reply::Ok(v) = &first {
            if v.get("status").and_then(Value::as_str) == Some("RUNNING") {
                if let Some(id) = v.get("evaluationId").and_then(Value::as_str) {
                    let poll = format!("/projects/{}/evaluations/{}", p.project, id);
                    for _ in 0..60 {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        match self.backend.call(Method::GET, &poll, None, held.as_deref(), false, None).await {
                            Reply::Ok(r) if r.get("status").and_then(Value::as_str) == Some("RUNNING") => {}
                            other => return self.finish(other, held.is_some()),
                        }
                    }
                    return Ok(CallToolResult::success(vec![ContentBlock::text(v.to_string())]));
                }
            }
        }
        self.finish(first, held.is_some())
    }

    #[tool(description = "Read the current Architecture Model, completeness map, open questions, \
        findings, and decisions for the project.")]
    async fn meridian_model(&self, p: Parameters<ModelParams>) -> Result<CallToolResult, McpError> {
        let p = p.0;
        self.forward(
            Method::GET,
            format!("/projects/{}/model?include=completeness,questions,findings,decisions", p.project),
            None,
            p.passphrase,
            false,
            None,
        )
        .await
    }
}

#[tool_handler]
impl ServerHandler for MeridianTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("meridian-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Meridian captures a project's architecture through an interview. Start with \
                 `meridian_projects` to find or create the project. Then call `meridian_next` \
                 repeatedly: it returns what is missing as questions. Answer from the repository \
                 where you can and ask the user where you cannot, then pass the answers back (as \
                 `answers` on `meridian_next`, or as facts with `meridian_submit`). Send observations \
                 about the project: paths, counts, short summaries, and the text of architecture documents. Tools that read or write \
                 the model need the user's Architecture Passphrase; if a tool returns \
                 `passphrase_required`, ask the user for it and retry. Never invent a passphrase.",
            )
    }
}
