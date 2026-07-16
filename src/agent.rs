// Transport Logic
use crate::models::{
    ArchitectureReviewReadiness, ArchitectureReviewRequest, AuthNResult,
    BitcoinFundingRequestResponse, BitcoinFundingStatusResponse, CreateAccountRequest,
    HealthHeartbeat, RequestApiKeyRequest,
};
use anyhow::{Context, Result};
use reqwest::{Client, Method, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

static CLIENT: OnceLock<Client> = OnceLock::new();
static SESSION: OnceLock<Arc<Mutex<Option<Session>>>> = OnceLock::new();

const LOGIN_PATH: &str = "/api/security/login";
const SESSION_REFRESH: &str = "/api/security/session/refresh";
const LOGOUT_PATH: &str = "/api/security/logout";
const CREATE_ACCOUNT_PATH: &str = "/api/user/create";
const API_KEY_PATH: &str = "/api/user/apiKey";
const HEALTH_HEARTBEAT_PATH: &str = "/api/health/heartbeat";
const BITCOIN_PAYMENT_REQUEST_PATH: &str = "/api/payment/request/bitcoin";
const BITCOIN_PAYMENT_STATUS_PATH: &str = "/api/payment/request/bitcoin/status";
const REVIEW_PATH: &str = "/api/skills/review";
const SESSION_EXPIRY_SAFETY_MARGIN_MILLIS: u64 = 30_000;
const MAX_RETRY_ATTEMPTS: usize = 5;
const RETRY_DELAYS_MILLIS: [u64; 5] = [500, 1_000, 2_000, 4_000, 8_000];

#[derive(Debug, Clone, Serialize)]
struct AuthNRequest {
    #[serde(rename = "rawKey")]
    raw_key: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    username: Option<String>,
    phone: Option<String>,
    email: Option<String>,
    password: Option<String>,
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

fn session_bearer_token(session_id: &str) -> String {
    let session_id = session_id.trim();

    if session_id.starts_with("m_session_") {
        session_id.to_string()
    } else {
        format!("m_session_{session_id}")
    }
}

fn retry_delay(attempt: usize) -> Duration {
    let index = attempt.saturating_sub(1).min(RETRY_DELAYS_MILLIS.len() - 1);
    Duration::from_millis(RETRY_DELAYS_MILLIS[index])
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
    )
}

fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.is_request()
}

async fn wait_before_retry(attempt: usize) {
    tokio::time::sleep(retry_delay(attempt)).await;
}

async fn send_with_retry(
    method: Method,
    url: &str,
    bearer_token: Option<String>,
    body: Option<Value>,
) -> Result<Response> {
    let mut last_error: Option<reqwest::Error> = None;

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        let mut request = http().request(method.clone(), url);

        if let Some(token) = bearer_token.as_ref() {
            request = request.bearer_auth(token);
        }

        if let Some(payload) = body.as_ref() {
            request = request.json(payload);
        }

        match request.send().await {
            Ok(response) => {
                if !is_retryable_status(response.status()) || attempt == MAX_RETRY_ATTEMPTS {
                    return Ok(response);
                }

                wait_before_retry(attempt).await;
            }
            Err(error) => {
                if !is_retryable_error(&error) || attempt == MAX_RETRY_ATTEMPTS {
                    return Err(error).context("failed to reach backend");
                }

                last_error = Some(error);
                wait_before_retry(attempt).await;
            }
        }
    }

    match last_error {
        Some(error) => Err(error).context("failed to reach backend after retries"),
        None => anyhow::bail!("backend remained unavailable after retries"),
    }
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
        raw_key: Some(api_key.trim().to_string()),
        session_id: None,
        username: None,
        phone: None,
        email: None,
        password: None,
    };

    let response = send_with_retry(
        Method::POST,
        &url,
        None,
        Some(serde_json::to_value(&body).context("failed to serialize backend login request")?),
    )
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
    let bearer_token = session_bearer_token(current_session_id);

    let response = send_with_retry(Method::POST, &url, Some(bearer_token), None)
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
    let bearer_token = session_bearer_token(&current_session_id);

    let response = send_with_retry(Method::POST, &url, Some(bearer_token), None)
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

pub async fn create_account(request: CreateAccountRequest) -> Result<()> {
    let backend_url = crate::config::backend_url()?;
    let url = format!("{backend_url}{CREATE_ACCOUNT_PATH}");

    let response = send_with_retry(
        Method::POST,
        &url,
        None,
        Some(serde_json::to_value(&request).context("failed to serialize backend account creation request")?),
    )
    .await
    .context("failed to reach backend account creation endpoint")?;

    match response.status() {
        s if s.is_success() => {
            crate::config::clear_api_key()
                .context("account was created, clear any existing local API key")?;
            invalidate_session().await;
            Ok(())
        }
        reqwest::StatusCode::CONFLICT => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("account already exists or conflicts with an existing account: {body}")
        }
        reqwest::StatusCode::BAD_REQUEST => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("account creation request rejected: {body}")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend account creation error {status}: {body}")
        }
    }
}

pub async fn request_and_save_api_key(request: RequestApiKeyRequest) -> Result<String> {
    let backend_url = crate::config::backend_url()?;

    let login_response =
        login_with_username_password(&backend_url, &request.username, &request.password).await?;

    let raw_api_key =
        request_api_key_with_session(&backend_url, &login_response.session_id).await?;

    crate::config::set_api_key(&raw_api_key)?;

    Ok(raw_api_key)
}

pub async fn request_bitcoin_funding(amount_sats: u64) -> Result<BitcoinFundingRequestResponse> {
    if amount_sats == 0 {
        anyhow::bail!("amountSats must be greater than zero");
    }

    let backend_url = crate::config::backend_url()?;
    let url = format!("{backend_url}{BITCOIN_PAYMENT_REQUEST_PATH}?amountSats={amount_sats}");

    let response = authenticated_get(&url).await?;

    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend Bitcoin payment request response"),
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        reqwest::StatusCode::PAYMENT_REQUIRED => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Bitcoin funding request requires payment setup: {body}")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend Bitcoin payment request error {status}: {body}")
        }
    }
}

pub async fn bitcoin_funding_status(address: &str) -> Result<BitcoinFundingStatusResponse> {
    if address.trim().is_empty() {
        anyhow::bail!("address must not be empty");
    }

    let backend_url = crate::config::backend_url()?;
    let address = urlencoding::encode(address.trim());
    let url = format!("{backend_url}{BITCOIN_PAYMENT_STATUS_PATH}/{address}");

    let response = authenticated_get(&url).await?;

    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend Bitcoin payment status response"),
        reqwest::StatusCode::UNAUTHORIZED => {
            invalidate_session().await;
            anyhow::bail!("backend session expired or was rejected after re-login")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend Bitcoin payment status error {status}: {body}")
        }
    }
}

async fn login_with_username_password(
    backend_url: &str,
    username: &str,
    password: &str,
) -> Result<AuthNResult> {
    let url = format!("{backend_url}{LOGIN_PATH}");
    let body = AuthNRequest {
        raw_key: None,
        session_id: None,
        username: Some(username.to_string()),
        phone: None,
        email: None,
        password: Some(password.to_string()),
    };

    let response = send_with_retry(
        Method::POST,
        &url,
        None,
        Some(serde_json::to_value(&body).context("failed to serialize backend login request")?),
    )
    .await
    .context("failed to reach backend login endpoint")?;

    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend login response"),
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("invalid username or password")
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

async fn request_api_key_with_session(backend_url: &str, session_id: &str) -> Result<String> {
    let url = format!("{backend_url}{API_KEY_PATH}");

    let response = send_with_retry(
            Method::GET,
            &url,
            Some(session_bearer_token(session_id)),
            None,
        )
        .await
        .context("failed to reach backend API key endpoint")?;

    match response.status() {
        s if s.is_success() => {
            let raw_api_key = response
                .text()
                .await
                .context("failed to read backend API key response")?;

            if raw_api_key.trim().is_empty() {
                anyhow::bail!("backend returned an empty API key")
            }

            Ok(raw_api_key)
        }
        reqwest::StatusCode::CONFLICT => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "an active API key already exists. Continue using the original configured key. Only call request_api_key again if the original key becomes inactive: {body}"
            )
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("backend rejected the login session while requesting an API key")
        }
        status => {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("backend API key request error {status}: {body}")
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
    body: &ArchitectureReviewRequest,
) -> Result<reqwest::Response> {
    let bearer_token = session_bearer_token(session_id);
    send_with_retry(
            Method::POST,
            url,
            Some(bearer_token),
            Some(serde_json::to_value(body).context("failed to serialize backend review request")?),
        )
        .await
}

async fn send_authenticated_get(url: &str, session_id: &str) -> Result<reqwest::Response> {
    let bearer_token = session_bearer_token(session_id);
    send_with_retry(Method::GET, url, Some(bearer_token), None).await
}

async fn authenticated_get(url: &str) -> Result<reqwest::Response> {
    let api_key = crate::config::api_key()?;
    let backend_url = crate::config::backend_url()?;

    let mut current_session_id = session_id(&api_key, &backend_url).await?;
    let mut response = send_authenticated_get(url, &current_session_id).await?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        invalidate_session().await;
        current_session_id = login(&api_key, &backend_url).await?;
        response = send_authenticated_get(url, &current_session_id).await?;
    }

    Ok(response)
}

async fn post_review_stage(
    path: &str,
    body: &ArchitectureReviewRequest,
) -> Result<reqwest::Response> {
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

async fn parse_review_response(response: reqwest::Response) -> Result<Value> {
    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend review response"),
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

async fn parse_readiness_response(
    response: reqwest::Response,
) -> Result<ArchitectureReviewReadiness> {
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

/// Test backend reachability using the unsecured health heartbeat endpoint.
pub async fn test_backend_health() -> Result<HealthHeartbeat> {
    let backend_url = crate::config::backend_url()?;
    let url = format!("{backend_url}{HEALTH_HEARTBEAT_PATH}");

    let response = match send_with_retry(Method::GET, &url, None, None).await {
        Ok(response) => response,
        Err(_) => {
            return Ok(HealthHeartbeat {
                status: "DOWN".to_string(),
                timestamp: humantime::format_rfc3339_nanos(SystemTime::now()).to_string(),
            });
        }
    };

    match response.status() {
        s if s.is_success() => response
            .json()
            .await
            .context("failed to parse backend health heartbeat response"),
        _ => Ok(response.json().await.unwrap_or_else(|_| HealthHeartbeat {
            status: "DOWN".to_string(),
            timestamp: humantime::format_rfc3339_nanos(SystemTime::now()).to_string(),
        })),
    }
}

/// Test backend login and return the AuthNResult.
pub async fn test_login() -> Result<AuthNResult> {
    let api_key = crate::config::api_key()?;
    let backend_url = crate::config::backend_url()?;

    login_result(&api_key, &backend_url).await
}

pub async fn run_review(request: &ArchitectureReviewRequest) -> Result<Value> {
    let response = post_review_stage(REVIEW_PATH, request).await?;
    parse_review_response(response).await
}
