use crate::scanner::ArchModel;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

static CLIENT: OnceLock<Client> = OnceLock::new();
static SESSION: OnceLock<Arc<Mutex<Option<Session>>>> = OnceLock::new();

const LOGIN_PATH: &str = "/security/login";
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity:      String,
    #[serde(rename = "type")]
    pub finding_type:  String,
    pub file:          String,
    pub line:          Option<u32>,
    pub title:         String,
    pub explanation:   String,
    pub consequence:   String,
    pub suggestion:    String,
    pub adr_reference: Option<String>,
    pub confidence:    f64,
}

#[derive(Serialize)]
struct ReviewRequest<'a> {
    arch_model: &'a ArchModel,
    file_path:  &'a str,
    content:    &'a str,
}

#[derive(Deserialize)]
struct ReviewResponse {
    findings: Vec<Finding>,
}

#[derive(Deserialize)]
struct LoginResponse {
    session_id: String,
    expires_at: u64,
}

async fn session_id(api_key: &str, backend_url: &str) -> Result<String> {
    {
        let session = session_cache().lock().await;
        if let Some(session) = session.as_ref().filter(|session| session.is_valid()) {
            return Ok(session.session_id.clone());
        }
    }

    login(api_key, backend_url).await
}

async fn login(api_key: &str, backend_url: &str) -> Result<String> {
    let url = format!("{backend_url}{LOGIN_PATH}");

    let response = http()
        .post(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .context("failed to reach backend login endpoint")?;

    match response.status() {
        s if s.is_success() => {
            let data: LoginResponse = response
                .json()
                .await
                .context("failed to parse backend login response")?;

            let session = Session {
                session_id: data.session_id.clone(),
                expires_at: data.expires_at,
            };

            let mut cached_session = session_cache().lock().await;
            *cached_session = Some(session);

            Ok(data.session_id)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("invalid API key — check MERIDIAN_API_KEY")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend login error {status}: {body}")
        }
    }
}

async fn invalidate_session() {
    let mut session = session_cache().lock().await;
    *session = None;
}

async fn send_review_request(
    url: &str,
    session_id: &str,
    body: &ReviewRequest<'_>,
) -> Result<reqwest::Response> {
    http()
        .post(url)
        .bearer_auth(session_id)
        .json(body)
        .send()
        .await
        .context("failed to reach backend")
}

/// Send a review request to the Java backend and return findings.
pub async fn review(
    model:     &ArchModel,
    file_path: &str,
    content:   &str,
) -> Result<Vec<Finding>> {
    let api_key = crate::config::api_key()?;
    let backend_url  = std::env::var("MERIDIAN_BACKEND_URL")
        .unwrap_or_else(|_| "https://resolvingarchitecture.io/meridian/api".to_string());

    let url  = format!("{backend_url}/api/review");
    let body = ReviewRequest { arch_model: model, file_path, content };

    let mut current_session_id = session_id(&api_key, &backend_url).await?;
    let mut response = send_review_request(&url, &current_session_id, &body).await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        invalidate_session().await;
        current_session_id = login(&api_key, &backend_url).await?;
        response = send_review_request(&url, &current_session_id, &body).await?;
    }

    match response.status() {
        s if s.is_success() => {
            let data: ReviewResponse = response
                .json()
                .await
                .context("failed to parse backend response")?;
            Ok(data.findings)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => {
            anyhow::bail!("monthly review limit reached — visit https://resolvingarchitecture.io/meridian")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend error {status}: {body}")
        }
    }
}
