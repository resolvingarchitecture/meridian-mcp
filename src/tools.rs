//! The MCP tool surface (`ADR-AppArch-25`): `meridian_next`, `meridian_submit`,
//! `meridian_evaluate`, `meridian_model`. Each forwards to the backend.

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Duration;

use crate::backend::{Backend, Reply};
use crate::session::Session;

pub struct MeridianTools {
    backend: Backend,
    session: Session,
}

/// What kind of finding an observation is.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationKind {
    Repository,
    Module,
    Dependency,
    EntryPoint,
    Build,
    Deployment,
    Api,
    DataStore,
    Document,
    Diagram,
}

/// A structured finding about the project, found by looking at it locally.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// Optional. If you leave it out, a stable one is made from the kind and path, so sending the
    /// same finding again does not duplicate it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// What kind of finding this is. Use the field name `kind`.
    #[serde(alias = "type")]
    pub kind: ObservationKind,
    /// Repository-relative path, when the finding has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A short description of the finding, at most 2000 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Small scalar values and counts, at most 32 keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Map<String, Value>>,
    /// A hash of the underlying content, so unchanged findings can be recognized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// For `DOCUMENT` and `DIAGRAM` only: the text of the architecture document (an ADR, a design doc,
    /// diagram source), up to 256 KiB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// An answer to a question that `meridian_next` returned.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// The `questionId` of the question you are answering, for example `q-RISK-1`.
    pub question_id: String,
    /// The answer. Follow the question's `answerExample` and `answerFormat`: an object like the
    /// example, a list of such objects, or, when `answerFormat` says so, a plain string.
    pub value: Value,
}

/// The kind of architecture fact being submitted.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FactCategory {
    Scope,
    BusinessGoal,
    Stakeholder,
    Concern,
    QualityAttribute,
    Constraint,
    Component,
    Relationship,
    Decision,
    Risk,
    Standard,
}

/// Where a fact came from.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FactSource {
    CodeScan,
    UserAnswer,
    ImportedDocument,
}

/// An architecture fact to record. It enters the model directly.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    /// The kind of fact.
    pub category: FactCategory,
    /// The fact itself, as an object shaped like the `answerExample` Meridian gives for that category.
    pub data: serde_json::Map<String, Value>,
    /// Where it came from. Defaults to `USER_ANSWER`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<FactSource>,
    /// How sure you are, 0 to 1. Kept as a note; Meridian assigns its own confidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_confidence: Option<f64>,
    /// IDs of the observations this fact came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_ids: Option<Vec<String>>,
    /// The `questionId` this answers, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct NextParams {
    /// Meridian project (architecture context) ID, from `meridian_projects`.
    pub project: String,
    /// Findings about the project: paths, counts, short summaries, and the text of architecture documents.
    #[serde(default)]
    pub observations: Vec<Observation>,
    /// Answers to questions from an earlier call. They enter the model directly.
    #[serde(default)]
    pub answers: Vec<Answer>,
    /// The user's Architecture Passphrase. Ask the user if you do not have it.
    pub passphrase: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct SubmitParams {
    pub project: String,
    /// Facts to record. They enter the Architecture Model directly; there is no approval step.
    pub facts: Vec<Fact>,
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
        and get back the current phase, the gaps in the architecture, and the next questions. Every \
        question carries an `answerExample` and an `answerFormat`: answer in that shape. Call \
        repeatedly until `done` is true.")]
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
                 `answers` on `meridian_next`, or as facts with `meridian_submit`). Every question comes with an \
                 `answerExample`; answer in that shape. Send observations \
                 about the project: paths, counts, short summaries, and the text of architecture documents. Tools that read or write \
                 the model need the user's Architecture Passphrase; if a tool returns \
                 `passphrase_required`, ask the user for it and retry. Never invent a passphrase.",
            )
    }
}
