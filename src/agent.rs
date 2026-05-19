use crate::scanner::ArchModel;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use uuid::Uuid;

static CLIENT: OnceLock<Client> = OnceLock::new();
static SESSION: OnceLock<Arc<Mutex<Option<Session>>>> = OnceLock::new();

const LOGIN_PATH: &str = "/api/security/login";
const SESSION_REFRESH: &str = "/api/security/session/refresh";
const LOGOUT_PATH: &str = "/api/security/logout";
const HEALTH_HEARTBEAT_PATH: &str = "/api/health/heartbeat";
const CONTEXT_PATH: &str = "/api/context";
const FULL_REVIEW_PROMPT_PATH: &str = "/api/skills/review/full/prompt";
const FULL_REVIEW_PATH: &str = "/api/skills/review/full";
const INTERMEDIATE_REVIEW_PATH: &str = "/api/skills/review/intermediate";
const SESSION_EXPIRY_SAFETY_MARGIN_MILLIS: u64 = 30_000;

fn http() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client")
    })
}

fn session_cache() -> &'static Arc<Mutex<Option<Session>>> {
    SESSION.get_or_init(|| Arc::new(Mutex::new(None)))
}

fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Clone)]
struct Session {
    session_id: String,
    expires_at: u64,
}

impl Session {
    fn is_valid(&self) -> bool {
        now_epoch_millis() + SESSION_EXPIRY_SAFETY_MARGIN_MILLIS < self.expires_at
    }
}

// ── Finding — matches Java backend JSON schema exactly ───────────────────────
#[derive(Serialize)]
struct ContentEnrichmentRequest {
    #[serde(rename = "request_id")]
    request_id: Uuid,
    context: ArchitectureContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureContext {
    #[serde(rename = "context_id")]
    pub context_id: Option<Uuid>,
    #[serde(rename = "organization_context")]
    pub organization_context: Option<serde_json::Value>,
    #[serde(rename = "business_goals")]
    pub business_goals: Option<Vec<String>>,
    pub stakeholders: Option<Vec<serde_json::Value>>,
    pub decisions: Option<Vec<serde_json::Value>>,
    pub constraints: Option<Vec<String>>,
    pub risks: Option<Vec<String>>,
    pub standards: Option<Vec<String>>,
    #[serde(rename = "scope_notes")]
    pub scope_notes: Option<Vec<String>>,
    #[serde(rename = "freeform_notes")]
    pub freeform_notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ContextResponse {
    #[serde(rename = "contextId")]
    pub context_id: Uuid,
    #[serde(rename = "contextPercentUsed")]
    pub context_percent_used: serde_json::Value,
    pub message: String,
}

#[derive(Serialize)]
struct MultipleReviewRequest {
    #[serde(rename = "request_id")]
    request_id: Uuid,
    #[serde(rename = "context_id")]
    context_id: Uuid,
    documents: Vec<DocumentInput>,
    options: ReviewOptions,
}

#[derive(Serialize)]
struct DocumentInput {
    id: String,
    title: String,
    filename: String,
    #[serde(rename = "type_hint")]
    type_hint: DocumentTypeHint,
    author: Option<String>,
    date: Option<String>,
    version: Option<String>,
    #[serde(rename = "stated_scope")]
    stated_scope: Option<String>,
    #[serde(rename = "organization_context")]
    organization_context: Option<serde_json::Value>,
    #[serde(rename = "known_stakeholders")]
    known_stakeholders: Vec<serde_json::Value>,
    #[serde(rename = "known_decisions")]
    known_decisions: Vec<serde_json::Value>,
    content: Vec<DocumentContent>,
}

#[derive(Serialize)]
struct DocumentContent {
    #[serde(rename = "content_type")]
    content_type: ContentType,
    #[serde(rename = "media_type")]
    media_type: String,
    encoding: ContentEncoding,
    data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum DocumentTypeHint {
    ApplicationDesign,
    ArchitectureDecisionRecord,
    IntegrationDesign,
    DataModel,
    InfrastructureDesign,
    SecurityDesign,
    ThreatModel,
    EnterpriseRoadmap,
    StandardsDocument,
    Runbook,
    Codebase,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ContentType {
    Text,
    Base64Pdf,
    Base64Img,
    Url,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ContentEncoding {
    Plain,
    Base64,
    Utf8,
}

#[derive(Serialize)]
struct ReviewOptions {
    #[serde(rename = "infer_stakeholders")]
    infer_stakeholders: bool,
    #[serde(rename = "infer_architectural_decisions")]
    infer_architectural_decisions: bool,
    #[serde(rename = "include_quality_attribute_ranking")]
    include_quality_attribute_ranking: bool,
    #[serde(rename = "domains_to_review")]
    domains_to_review: Vec<Domain>,
    #[serde(rename = "minimum_confidence_threshold")]
    minimum_confidence_threshold: f64,
    #[serde(rename = "minimum_gap_severity")]
    minimum_gap_severity: GapSeverity,
}

impl ReviewOptions {
    fn default_options() -> Self {
        Self {
            infer_stakeholders: true,
            infer_architectural_decisions: true,
            include_quality_attribute_ranking: true,
            domains_to_review: vec![
                Domain::Application,
                Domain::Integration,
                Domain::Data,
                Domain::Infrastructure,
                Domain::Security,
                Domain::Enterprise,
            ],
            minimum_confidence_threshold: 0.0,
            minimum_gap_severity: GapSeverity::Low,
        }
    }

    fn intermediate_options() -> Self {
        Self {
            infer_stakeholders: false,
            infer_architectural_decisions: false,
            include_quality_attribute_ranking: false,
            domains_to_review: vec![
                Domain::Application,
                Domain::Integration,
                Domain::Data,
                Domain::Infrastructure,
                Domain::Security,
                Domain::Enterprise,
            ],
            minimum_confidence_threshold: 0.4,
            minimum_gap_severity: GapSeverity::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum GapSeverity {
    Low,
    Medium,
    High,
}

fn build_multiple_review_request(
    model: &ArchModel,
    file_path: &str,
    content: &str,
    options: ReviewOptions,
) -> MultipleReviewRequest {
    let arch_model_json = serde_json::to_string_pretty(model).unwrap_or_else(|_| "{}".to_string());

    MultipleReviewRequest {
        request_id: Uuid::new_v4(),
        context_id: Uuid::new_v4(),
        documents: vec![
            DocumentInput {
                id: "architecture-model".to_string(),
                title: "Architecture model".to_string(),
                filename: "architecture-model.json".to_string(),
                type_hint: DocumentTypeHint::ApplicationDesign,
                author: None,
                date: None,
                version: None,
                stated_scope: Some("Locally scanned Meridian architecture model".to_string()),
                organization_context: None,
                known_stakeholders: Vec::new(),
                known_decisions: Vec::new(),
                content: vec![DocumentContent {
                    content_type: ContentType::Text,
                    media_type: "application/json".to_string(),
                    encoding: ContentEncoding::Utf8,
                    data: arch_model_json,
                }],
            },
            DocumentInput {
                id: file_path.to_string(),
                title: file_path.to_string(),
                filename: file_path.to_string(),
                type_hint: DocumentTypeHint::Codebase,
                author: None,
                date: None,
                version: None,
                stated_scope: Some("Source file submitted for architecture review".to_string()),
                organization_context: None,
                known_stakeholders: Vec::new(),
                known_decisions: Vec::new(),
                content: vec![DocumentContent {
                    content_type: ContentType::Code,
                    media_type: "text/plain".to_string(),
                    encoding: ContentEncoding::Utf8,
                    data: content.to_string(),
                }],
            },
        ],
        options,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: String,
    #[serde(rename = "type")]
    pub finding_type: String,
    pub file: String,
    pub line: Option<u32>,
    pub title: String,
    pub explanation: String,
    pub consequence: String,
    pub suggestion: String,
    pub adr_reference: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Domain {
    Application,
    Integration,
    Data,
    Infrastructure,
    Security,
    Enterprise,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComplexityModifier {
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEstimate {
    pub domain: Domain,
    pub present: bool,
    pub estimated_components: i32,
    pub complexity_modifier: ComplexityModifier,
    pub estimated_price: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainSelectionPrompt {
    #[serde(rename = "context_id")]
    pub context_id: String,
    pub status: String,
    pub question: String,
    #[serde(rename = "domain_estimates")]
    pub domain_estimates: Vec<DomainEstimate>,
    #[serde(rename = "sats_available")]
    pub sats_available: u64,
    #[serde(rename = "total_estimated_price")]
    pub total_estimated_price: u64,
    #[serde(rename = "requires_user_selection")]
    pub requires_user_selection: bool,
}

impl DomainSelectionPrompt {
    pub fn present_estimated_price(&self) -> u64 {
        self.domain_estimates
            .iter()
            .filter(|estimate| estimate.present)
            .map(|estimate| estimate.estimated_price)
            .sum()
    }

    pub fn selection_guidance(&self) -> Option<&'static str> {
        if self.requires_user_selection {
            Some("Present domain_estimates to the user. The selected domains' combined estimated_price must be less than or equal to sats_available. If the selection exceeds sats_available, ask the user to choose fewer domains or add more funds.")
        } else {
            None
        }
    }

    pub fn present_domains_exceed_available_balance(&self) -> bool {
        self.present_estimated_price() > self.sats_available
    }
}

#[derive(Deserialize)]
struct ReviewResponse {
    findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthHeartbeat {
    pub status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthNResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: u64,
    pub status: Option<i32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AuthNRequest {
    #[serde(rename = "rawKey")]
    raw_key: String,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    username: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    password: Option<String>,
}

async fn session_id(api_key: &str, backend_url: &str) -> Result<String> {
    let refreshable_session = {
        let session = session_cache().lock().await;
        match session.as_ref() {
            Some(session) if session.is_valid() => {
                return Ok(session.session_id.clone());
            }
            Some(session) => Some(session.session_id.clone()),
            None => None,
        }
    };

    if let Some(current_session_id) = refreshable_session {
        match refresh_session(&current_session_id, backend_url).await {
            Ok(new_session_id) => return Ok(new_session_id),
            Err(_) => invalidate_session().await,
        }
    }

    login(api_key, backend_url).await
}

async fn login(api_key: &str, backend_url: &str) -> Result<String> {
    let data = login_result(api_key, backend_url).await?;
    Ok(data.session_id)
}

async fn login_result(api_key: &str, backend_url: &str) -> Result<AuthNResult> {
    let url = format!("{backend_url}{LOGIN_PATH}");
    let body = AuthNRequest {
        raw_key: api_key.trim().to_string(),
        session_id: None,
        username: None,
        phone: None,
        email: None,
        password: None,
    };

    let response = http()
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("failed to reach backend login endpoint")?;

    match response.status() {
        s if s.is_success() => {
            let data: AuthNResult = response
                .json()
                .await
                .context("failed to parse backend login response")?;

            let session = Session {
                session_id: data.session_id.clone(),
                expires_at: data.expires_at,
            };

            let mut cached_session = session_cache().lock().await;
            *cached_session = Some(session);

            Ok(data)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("invalid API key — check MERIDIAN_API_KEY")
        }
        reqwest::StatusCode::FORBIDDEN => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend login forbidden: {body}")
        }
        reqwest::StatusCode::BAD_REQUEST => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend login request rejected: {body}")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend login error {status}: {body}")
        }
    }
}

async fn refresh_session(current_session_id: &str, backend_url: &str) -> Result<String> {
    let url = format!("{backend_url}{SESSION_REFRESH}");

    let response = http()
        .post(&url)
        .bearer_auth(current_session_id)
        .send()
        .await
        .context("failed to reach backend session refresh endpoint")?;

    match response.status() {
        s if s.is_success() => {
            let data: AuthNResult = response
                .json()
                .await
                .context("failed to parse backend session refresh response")?;

            let session = Session {
                session_id: data.session_id.clone(),
                expires_at: data.expires_at,
            };

            let mut cached_session = session_cache().lock().await;
            *cached_session = Some(session);

            Ok(data.session_id)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("backend session refresh rejected current session")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend session refresh error {status}: {body}")
        }
    }
}

pub async fn logout() -> Result<()> {
    let backend_url = crate::config::backend_url()?;

    let cached_session_id = {
        let session = session_cache().lock().await;
        session.as_ref().map(|session| session.session_id.clone())
    };

    let Some(current_session_id) = cached_session_id else {
        return Ok(());
    };

    let url = format!("{backend_url}{LOGOUT_PATH}");

    let response = http()
        .post(&url)
        .bearer_auth(&current_session_id)
        .send()
        .await
        .context("failed to reach backend logout endpoint")?;

    invalidate_session().await;

    match response.status() {
        s if s.is_success() => Ok(()),
        reqwest::StatusCode::UNAUTHORIZED => Ok(()),
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend logout error {status}: {body}")
        }
    }
}

async fn invalidate_session() {
    let mut session = session_cache().lock().await;
    *session = None;
}

async fn send_backend_request(
    url: &str,
    session_id: &str,
    body: &MultipleReviewRequest,
) -> Result<reqwest::Response> {
    http()
        .post(url)
        .bearer_auth(session_id)
        .json(body)
        .send()
        .await
        .context("failed to reach backend")
}

async fn send_context_request(
    url: &str,
    session_id: &str,
    body: &ContentEnrichmentRequest,
) -> Result<reqwest::Response> {
    http()
        .post(url)
        .bearer_auth(session_id)
        .json(body)
        .send()
        .await
        .context("failed to reach backend context endpoint")
}

async fn post_context(body: &ContentEnrichmentRequest) -> Result<reqwest::Response> {
    let api_key = crate::config::api_key()?;
    let backend_url = crate::config::backend_url()?;

    let url = format!("{backend_url}{CONTEXT_PATH}");

    let mut current_session_id = session_id(&api_key, &backend_url).await?;
    let mut response = send_context_request(&url, &current_session_id, body).await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        invalidate_session().await;
        current_session_id = login(&api_key, &backend_url).await?;
        response = send_context_request(&url, &current_session_id, body).await?;
    }

    Ok(response)
}

async fn post_review_stage(path: &str, body: &MultipleReviewRequest) -> Result<reqwest::Response> {
    let api_key = crate::config::api_key()?;
    let backend_url = crate::config::backend_url()?;

    let url = format!("{backend_url}{path}");

    let mut current_session_id = session_id(&api_key, &backend_url).await?;
    let mut response = send_backend_request(&url, &current_session_id, body).await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        invalidate_session().await;
        current_session_id = login(&api_key, &backend_url).await?;
        response = send_backend_request(&url, &current_session_id, body).await?;
    }

    Ok(response)
}

async fn parse_review_response(response: reqwest::Response) -> Result<Vec<Finding>> {
    match response.status() {
        s if s.is_success() => {
            let data: ReviewResponse = response
                .json()
                .await
                .context("failed to parse backend review response")?;
            Ok(data.findings)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            anyhow::bail!(
                "monthly review limit reached — visit https://resolvingarchitecture.io/meridian"
            )
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend review error {status}: {body}")
        }
    }
}

async fn parse_prompt_response(response: reqwest::Response) -> Result<DomainSelectionPrompt> {
    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend domain selection prompt response"),
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            anyhow::bail!(
                "monthly review limit reached — visit https://resolvingarchitecture.io/meridian"
            )
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend prompt error {status}: {body}")
        }
    }
}

async fn parse_context_response(response: reqwest::Response) -> Result<ContextResponse> {
    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend context response"),
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        reqwest::StatusCode::PAYMENT_REQUIRED => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("insufficient sats to add context: {body}")
        }
        reqwest::StatusCode::PAYLOAD_TOO_LARGE => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("context exceeds maximum size: {body}")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend context error {status}: {body}")
        }
    }
}

/// Test backend reachability using the unsecured health heartbeat endpoint.
pub async fn test_backend_health() -> Result<HealthHeartbeat> {
    let backend_url = crate::config::backend_url()?;
    let url = format!("{backend_url}{HEALTH_HEARTBEAT_PATH}");

    let response = http()
        .get(&url)
        .send()
        .await
        .context("failed to reach backend health heartbeat endpoint")?;

    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend health heartbeat response"),
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend health heartbeat error {status}: {body}")
        }
    }
}

/// Test backend login and return the AuthNResult.
pub async fn test_login() -> Result<AuthNResult> {
    let api_key = crate::config::api_key()?;
    let backend_url = crate::config::backend_url()?;

    login_result(&api_key, &backend_url).await
}

/// Add persistent architecture context to the backend.
///
/// The returned context_id can be included in subsequent review requests.
pub async fn add_context(context: ArchitectureContext) -> Result<ContextResponse> {
    let body = ContentEnrichmentRequest {
        request_id: Uuid::new_v4(),
        context,
    };

    let response = post_context(&body).await?;
    parse_context_response(response).await
}

/// Stage 1: build the full-review prompt.
///
/// This must be called before requesting a full review.
pub async fn build_full_review_prompt(
    model: &ArchModel,
    file_path: &str,
    content: &str,
) -> Result<DomainSelectionPrompt> {
    let body =
        build_multiple_review_request(model, file_path, content, ReviewOptions::default_options());
    let response = post_review_stage(FULL_REVIEW_PROMPT_PATH, &body).await?;
    parse_prompt_response(response).await
}

/// Stage 2: execute a full review.
///
/// This must be called after the full-review prompt stage.
pub async fn run_full_review(
    model: &ArchModel,
    file_path: &str,
    content: &str,
) -> Result<Vec<Finding>> {
    let body =
        build_multiple_review_request(model, file_path, content, ReviewOptions::default_options());
    let response = post_review_stage(FULL_REVIEW_PATH, &body).await?;
    parse_review_response(response).await
}

/// Stage 3: execute an intermediate review for a file change.
///
/// This must be called only after the full review stage has completed.
pub async fn run_intermediate_review(
    model: &ArchModel,
    file_path: &str,
    content: &str,
) -> Result<Vec<Finding>> {
    let body = build_multiple_review_request(
        model,
        file_path,
        content,
        ReviewOptions::intermediate_options(),
    );
    let response = post_review_stage(INTERMEDIATE_REVIEW_PATH, &body).await?;
    parse_review_response(response).await
}
