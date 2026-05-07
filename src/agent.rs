use crate::scanner::ArchModel;
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static CLIENT: OnceLock<Client> = OnceLock::new();

fn http() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client")
    })
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

/// Send a review request to the Java backend and return findings.
pub async fn review(
    model:     &ArchModel,
    file_path: &str,
    content:   &str,
) -> Result<Vec<Finding>> {
    let api_key      = std::env::var("MERIDIAN_API_KEY")
        .context("MERIDIAN_API_KEY not set")?;
    let backend_url  = std::env::var("MERIDIAN_BACKEND_URL")
        .unwrap_or_else(|_| "https://resolvingarchitecture.io/meridian/api".to_string());

    let url  = format!("{backend_url}/api/review");
    let body = ReviewRequest { arch_model: model, file_path, content };

    let response = http()
        .post(&url)
        .bearer_auth(&api_key)
        .json(&body)
        .send()
        .await
        .context("failed to reach backend")?;

    match response.status() {
        s if s.is_success() => {
            let data: ReviewResponse = response
                .json()
                .await
                .context("failed to parse backend response")?;
            Ok(data.findings)
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            anyhow::bail!("invalid API key — check MERIDIAN_API_KEY")
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
