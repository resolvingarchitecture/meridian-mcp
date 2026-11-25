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
    store: crate::store::Store,
    /// Why the credential store could not be read at startup, shown by `meridian_setup` so it is not silent.
    store_warning: Option<String>,
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
    /// A source, test, configuration or build file sent as evidence of what is built, so a review can
    /// compare the code with the design. Give its `path` and its `content`. It is stored encrypted under
    /// the project passphrase; sending the same path again replaces the earlier copy.
    Source,
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
    /// For `DOCUMENT`, `DIAGRAM` and `SOURCE`: the text of the file (an ADR, a design doc, diagram source,
    /// or a source file sent as evidence), up to 256 KiB.
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
    /// Service to run: `review_full` (the whole project) or `review_intermediate` (one change, compared with the
    /// architecture and the last full review). Other services are not available through the MCP yet.
    pub service: String,
    /// `review_intermediate` only: the git ref (branch, tag, or commit) to review all changes since, committed or
    /// not. When omitted, the uncommitted changes are reviewed, or the last commit when there are none.
    pub base: Option<String>,
    /// `review_intermediate` only: a few words saying what the change is for. Defaults to the last commit's subject
    /// when the last commit is the change.
    pub change_summary: Option<String>,
    /// Bring the project's stored source evidence up to date with the files on disk before the review: send the files
    /// Meridian already holds that have changed, and drop the ones that are gone. Defaults to true for `review_full`
    /// and false for `review_intermediate`, which updates the evidence from the change it just reviewed (it also
    /// reconciles by itself when history was rewritten).
    pub sync_evidence: Option<bool>,
    /// Directory to save a finished review's markdown report in, used as given. When omitted it is chosen for you:
    /// the `reviews/` folder of the git repository around `repo_path`, or, when `repo_path` holds several
    /// repositories, of its architecture, design, or admin folder, else the `reviews/` folder of `repo_path`; inside
    /// it, `full/` for a full review and `intermed/` for a change review.
    pub save_to: Option<String>,
    /// Repository root, used to read the project's version and to build the suggested location.
    /// Defaults to the server's working directory.
    pub repo_path: Option<String>,
    /// The project's version at the time of the review, recorded in the report. Detected from
    /// pom.xml, package.json, Cargo.toml, or pyproject.toml when omitted; pass it when the project
    /// has none of those.
    pub version: Option<String>,
    /// When true, return what the review would cost in credits without running it or spending anything. A review is
    /// priced from the size of the project's model and the evidence submitted, so ask before running one and tell the
    /// user the price when it is more than they expect.
    pub quote_only: Option<bool>,
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
    /// Starts with whatever the credential store holds for this backend: its API key if there is no key in the
    /// environment, else the claim secret of a pay-first sign-up still waiting for its payment.
    pub fn new(backend: Backend, store: crate::store::Store) -> Self {
        let session = Session::default();
        let mut store_warning = None;
        if !backend.has_api_key() {
            match store.load() {
                Ok(account) => {
                    if let Some(key) = account.api_key {
                        backend.set_api_key(key);
                    } else if let Some(signup) = account.signup {
                        session.set_signup(signup.id, signup.claim_secret);
                    }
                }
                Err(e) => store_warning = Some(e),
            }
        }
        Self { backend, session, store, store_warning }
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

    #[tool(description = "Get a person started with Meridian from nothing. Fully automatic, no email or captcha: \
        `signup_with_bitcoin` (N credits) returns a Bitcoin address and amount for the user to pay from their own \
        wallet, and `signup_status` then waits for the payment and, once it confirms, collects the API key and starts \
        using it. Or, with an email: action `create_account` signs them up \
        (email, password, optional username; Meridian emails a verification link they must open); `request_api_key` \
        signs in once and issues an API key, which this server then uses for the other tools (the key is shown \
        only once: tell the user to keep it); `buy_credits` starts a Bitcoin purchase of N credits and returns the \
        address and amount for the user to pay from their own wallet; `purchase_status` checks it (PAID when \
        confirmed). Ask the user to choose and supply the password; it is never stored. Needs no API key for the \
        first two actions.")]
    async fn meridian_setup(&self, p: Parameters<crate::setup::SetupParams>) -> Result<CallToolResult, McpError> {
        let outcome = crate::setup::run(&self.backend, &self.session, &self.store, p.0).await;
        let text = match outcome {
            Ok(mut v) => {
                if let Some(w) = &self.store_warning {
                    v["storageWarning"] = json!(w);
                }
                return Ok(CallToolResult::success(vec![ContentBlock::text(v.to_string())]));
            }
            Err(m) => match &self.store_warning {
                Some(w) => format!("{m} (Credential store: {w})"),
                None => m,
            },
        };
        Ok(CallToolResult::error(vec![ContentBlock::text(text)]))
    }

    #[tool(description = "Show the signed-in account: the user's name, how many credits they have and what one run of \
        each service costs (and whether they can afford it), how many reviews they have run, and how many projects \
        they have. Takes no arguments and needs no passphrase. Call it before `meridian_evaluate` to check the user \
        has enough credits.")]
    async fn meridian_account(&self) -> Result<CallToolResult, McpError> {
        self.forward(Method::GET, "/account".into(), None, None, false, None).await
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

    #[tool(description = "Run a Meridian review against the project: `review_full` for the whole project, or \
        `review_intermediate` for one change (the git diff since `base`, else since the last revision Meridian reviewed, \
        else the uncommitted changes, else the last commit), checked for architectural drift against the model and the last \
        full review; run a full review first. The diff is not stored; after the review the files it touched replace the \
        stored source evidence (deleted ones are removed), so the next change starts from where this one ended. \
        Waits for the result. A finished review comes back as a markdown \
        `report`, and is saved straight away: in `save_to` if given, otherwise in the `reviews/` \
        folder of the git repository it belongs in (or of an architecture, design, or admin folder when \
        the work spans several repositories), otherwise in the `reviews/` folder of `repo_path`; inside it, a full \
        review goes in `full/` and a change review in `intermed/`. The saved \
        file records the project's version at the time of the review; `savedTo` says where it went. A review is priced from the \
        size of the project, so call it first with `quote_only` to see the price and whether the user can afford \
        it (a change review is priced from the size of the change); credits are charged only if the review completes.")]
    async fn meridian_evaluate(
        &self,
        p: Parameters<EvaluateParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = p.0;
        let held = self.session.resolve(p.passphrase.clone());
        let intermediate = is_intermediate(&p.service);
        let service = if intermediate { "REVIEW_INTERMEDIATE".to_string() } else { p.service.clone() };
        let quote_only = p.quote_only.unwrap_or(false);
        let fail = |e: String| Ok(CallToolResult::error(vec![ContentBlock::text(e)]));

        // Source evidence is only touched for a real review, never for a quote.
        let sync_flag = !quote_only && p.sync_evidence.unwrap_or(!intermediate);
        let mut sync_report: Vec<Value> = Vec::new();
        let mut stored: Option<crate::change::StoredEvidence> = None;
        let root = if intermediate || sync_flag {
            match repo_root(p.repo_path.as_deref()) {
                Ok(root) => Some(root),
                Err(e) => return fail(e),
            }
        } else {
            None
        };
        if intermediate || sync_flag {
            match self.fetch_evidence(&p.project, held.as_deref()).await {
                Ok(e) => stored = Some(e),
                Err(reply) => return self.finish(reply, held.is_some()),
            }
        }

        // Reconcile what Meridian holds with the files on disk first, when asked (a full review's default).
        if sync_flag && !intermediate {
            if let (Some(root), Some(evidence)) = (&root, &stored) {
                match crate::change::reconcile(root, evidence) {
                    Ok(r) => sync_report.push(
                        self.push_evidence(&p.project, held.as_deref(), r.files, r.removed, Some(r.revision), "before the review").await,
                    ),
                    Err(e) => sync_report.push(json!({ "when": "before the review", "error": e })),
                }
            }
        }

        let mut change: Option<crate::change::Change> = None;
        let body = if intermediate {
            // A change review is priced from the change, so a quote needs it too. The diff is read from the local
            // repository here and sent for the one review; it is not stored.
            let root = root.as_ref().expect("a root is resolved for an intermediate review");
            let evidence_revision = stored.as_ref().and_then(|e| e.revision.clone());
            match crate::change::collect(root, p.base.as_deref(), evidence_revision.as_deref()) {
                Ok(c) => {
                    if c.base_missing && !quote_only {
                        // History was rewritten: make the evidence match the files as they are, then review what is left.
                        if let Some(evidence) = &stored {
                            match crate::change::reconcile(root, evidence) {
                                Ok(r) => sync_report.push(
                                    self.push_evidence(&p.project, held.as_deref(), r.files, r.removed, Some(r.revision), "history was rewritten").await,
                                ),
                                Err(e) => sync_report.push(json!({ "when": "history was rewritten", "error": e })),
                            }
                        }
                    }
                    let summary = p.change_summary.clone().or_else(|| c.summary.clone());
                    let mut options = json!({
                        "changes": c.diff,
                        "changeSummary": summary,
                        "revision": c.revision,
                        "baseRevision": c.base_revision,
                    });
                    if quote_only {
                        options["quoteOnly"] = json!(true);
                    }
                    change = Some(c);
                    json!({ "service": service, "options": options })
                }
                Err(e) => return fail(e),
            }
        } else if quote_only {
            json!({ "service": service, "options": { "quoteOnly": true } })
        } else {
            json!({ "service": service })
        };

        let path = format!("/projects/{}/evaluate", p.project);
        let first = self
            .backend
            .call(Method::POST, &path, Some(body), held.as_deref(), true, None)
            .await;
        // A 202 carries an evaluationId; poll until the result is ready.
        let mut result = first;
        if let Reply::Ok(v) = &result {
            if v.get("status").and_then(Value::as_str) == Some("RUNNING") {
                if let Some(id) = v.get("evaluationId").and_then(Value::as_str) {
                    let poll = format!("/projects/{}/evaluations/{}", p.project, id);
                    let mut finished = None;
                    // A review is one long model call; wait up to fifteen minutes for it.
                    for _ in 0..450 {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        match self.backend.call(Method::GET, &poll, None, held.as_deref(), false, None).await {
                            Reply::Ok(r) if r.get("status").and_then(Value::as_str) == Some("RUNNING") => {}
                            other => {
                                finished = Some(other);
                                break;
                            }
                        }
                    }
                    match finished {
                        Some(other) => result = other,
                        None => {
                            let mut still = v.clone();
                            still["note"] = json!(
                                "Still running after fifteen minutes. Call meridian_evaluate again to rejoin this \
                                 evaluation; it will not start a second one."
                            );
                            return Ok(CallToolResult::success(vec![ContentBlock::text(still.to_string())]));
                        }
                    }
                }
            }
        }

        // The review is done: bring the evidence up to date with the change it just looked at. A review that could not
        // run at all leaves the evidence alone, so the next one starts from the same revision.
        let reviewed = matches!(&result, Reply::Ok(v)
            if matches!(v.get("status").and_then(Value::as_str), Some("COMPLETE") | Some("NOT_READY")));
        let note = change.as_ref().and_then(|c| c.note.clone());
        let covered = change.as_ref().map(|c| c.described_as.clone());
        if reviewed && !quote_only {
            if let Some(c) = change.take() {
                sync_report.push(
                    self.push_evidence(&p.project, held.as_deref(), c.files, c.removed, Some(c.revision), "after the review").await,
                );
            }
        }
        if let Reply::Ok(v) = &mut result {
            if !sync_report.is_empty() {
                v["evidenceSync"] = Value::Array(sync_report);
            }
            if let Some(note) = note {
                v["note"] = json!(note);
            }
            if let Some(covered) = covered {
                v["changeReviewed"] = json!(covered);
            }
        }
        self.finish(save_review(result, &p), held.is_some())
    }

    /// The project's stored source evidence: what revision it reflects and each file's hash.
    async fn fetch_evidence(&self, project: &str, passphrase: Option<&str>) -> Result<crate::change::StoredEvidence, Reply> {
        match self.backend.call(Method::GET, &format!("/projects/{project}/evidence"), None, passphrase, false, None).await {
            Reply::Ok(v) => Ok(crate::change::StoredEvidence::from_json(&v)),
            other => Err(other),
        }
    }

    /// Sends files and removals to the backend in as many calls as the limits need; the revision goes with the last, so it
    /// only advances when everything before it was accepted. Returns what happened, for the result.
    async fn push_evidence(
        &self,
        project: &str,
        passphrase: Option<&str>,
        files: Vec<crate::change::EvidenceFile>,
        removed: Vec<String>,
        revision: Option<String>,
        when: &str,
    ) -> Value {
        let (mut updated, mut gone, mut rejected) = (0u64, 0u64, Vec::new());
        let mut recorded: Option<String> = None;
        let batches = crate::change::batches(files, removed);
        let last = batches.len() - 1;
        for (i, batch) in batches.into_iter().enumerate() {
            let body = json!({
                "revision": if i == last { revision.clone() } else { None },
                "files": batch.files.iter().map(|f| json!({ "path": f.path, "content": f.content, "contentHash": f.content_hash })).collect::<Vec<_>>(),
                "removed": batch.removed,
            });
            match self.backend.call(Method::POST, &format!("/projects/{project}/evidence/sync"), Some(body), passphrase, true, None).await {
                Reply::Ok(v) => {
                    updated += v.get("updated").and_then(Value::as_u64).unwrap_or(0);
                    gone += v.get("removed").and_then(Value::as_u64).unwrap_or(0);
                    if let Some(r) = v.get("rejected").and_then(Value::as_array) {
                        rejected.extend(r.iter().cloned());
                    }
                    if i == last {
                        recorded = v.get("revision").and_then(Value::as_str).map(str::to_owned);
                    }
                }
                Reply::Failed(e) => return json!({ "when": when, "error": e, "updated": updated, "removed": gone }),
                _ => return json!({ "when": when, "error": "the passphrase was not accepted for the evidence update", "updated": updated, "removed": gone }),
            }
        }
        json!({ "when": when, "updated": updated, "removed": gone, "revision": recorded, "rejected": rejected })
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
                "Meridian captures a project's architecture through an interview. If the user has no \
                 account or API key yet, start with `meridian_setup`. Start with \
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

/// Saves a completed review's markdown report: where `save_to` says, else in the `reviews/` of the git
/// repository (or architecture/design/admin folder) it belongs in, else the `reviews/` of `repo_path`. Adds `savedTo`, `projectVersion`,
/// and `versionSource`, or `saveError`. Any other reply passes through.
fn save_review(reply: Reply, p: &EvaluateParams) -> Reply {
    let Reply::Ok(mut v) = reply else { return reply };
    let complete = v.get("status").and_then(Value::as_str) == Some("COMPLETE");
    if !complete || v.get("report").and_then(Value::as_str).is_none() {
        return Reply::Ok(v);
    }
    let root = match repo_root(p.repo_path.as_deref()) {
        Ok(root) => root,
        Err(e) => {
            v["saveError"] = json!(e);
            return Reply::Ok(v);
        }
    };
    // Where it goes: the user's `save_to`, else the repository's (or architecture folder's) `reviews/`, else the
    // `reviews/` of the path itself.
    let dir = match &p.save_to {
        Some(dir) => std::path::PathBuf::from(dir),
        None => {
            let found = crate::review_report::resolve_destination(&root);
            v["destinationReason"] = json!(found.reason);
            // Full reviews and change reviews each have a folder inside `reviews/`.
            match v.get("service").and_then(Value::as_str).and_then(crate::review_report::subfolder) {
                Some(sub) => found.dir.join(sub),
                None => found.dir,
            }
        }
    };
    apply_save(&mut v, &dir, &root, p.version.as_deref(), &p.project);
    Reply::Ok(v)
}

/// Whether `service` names the intermediate (single change) review, in any of the spellings an agent is likely to use.
fn is_intermediate(service: &str) -> bool {
    let norm = service.trim().to_uppercase().replace(['-', ' '], "_");
    matches!(norm.as_str(), "REVIEW_INTERMEDIATE" | "INTERMEDIATE" | "INTERMEDIATE_REVIEW" | "CHANGE_REVIEW")
}

fn repo_root(repo_path: Option<&str>) -> Result<std::path::PathBuf, String> {
    match repo_path {
        Some(path) => Ok(std::path::PathBuf::from(path)),
        None => std::env::current_dir().map_err(|e| format!("no repository path and no working directory: {e}")),
    }
}

fn apply_save(v: &mut Value, dir: &std::path::Path, root: &std::path::Path, version: Option<&str>, project: &str) {
    match crate::review_report::save(dir, root, version, project, v) {
        Ok(saved) => {
            v["savedTo"] = json!(saved.path.display().to_string());
            v["projectVersion"] = json!(saved.version);
            v["versionSource"] = json!(saved.version_source);
        }
        Err(e) => v["saveError"] = json!(e),
    }
}

#[cfg(test)]
mod observation_tests {
    use super::*;

    #[test]
    fn a_source_observation_serializes_with_its_path_and_content() {
        let o: Observation = serde_json::from_value(serde_json::json!({
            "kind": "SOURCE", "path": "src/Bus.java", "content": "class Bus {}"
        }))
        .unwrap();
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["kind"], "SOURCE");
        assert_eq!(v["path"], "src/Bus.java");
        assert_eq!(v["content"], "class Bus {}");
    }
}

#[cfg(test)]
mod service_name_tests {
    use super::is_intermediate;

    #[test]
    fn the_intermediate_review_is_recognised_in_the_spellings_an_agent_uses() {
        for name in ["review_intermediate", "REVIEW_INTERMEDIATE", "intermediate", "Intermediate Review", "change-review"] {
            assert!(is_intermediate(name), "{name}");
        }
        for name in ["review_full", "review", "design", ""] {
            assert!(!is_intermediate(name), "{name}");
        }
    }
}
