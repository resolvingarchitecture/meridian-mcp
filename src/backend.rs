//! Thin HTTPS client for `meridian-backend`. The binary is stateless: the backend owns the
//! model, the gap rules, entitlements, and payments (`ADR-AppArch-25`).
//!
//! Wire contract: `meridian-docs/architecture/mcp-api.md` (v1).

use anyhow::{Context, Result};
use reqwest::Method;
use serde_json::Value;
use uuid::Uuid;

use crate::config::Config;

/// Outcome of a backend call.
pub enum Reply {
    Ok(Value),
    /// The backend needs the Architecture Passphrase and none was sent.
    PassphraseRequired,
    /// A passphrase was sent but is wrong.
    PassphraseIncorrect,
    /// The backend refused or failed; the message is safe to show the agent.
    Failed(String),
}

pub struct Backend {
    http: reqwest::Client,
    config: Config,
}

impl Backend {
    pub fn new(config: Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(client_id())
            .build()
            .context("building HTTP client")?;
        Ok(Self { http, config })
    }

    /// Call `{api_url}/mcp/v1{path}` and classify the reply.
    ///
    /// `body` is sent as JSON for non-GET methods. The passphrase goes in a header, never the body.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        passphrase: Option<&str>,
        idempotent: bool,
        if_match: Option<u64>,
    ) -> Reply {
        let url = format!("{}/mcp/v1{}", self.config.api_url.trim_end_matches('/'), path);
        let mut req = self
            .http
            .request(method.clone(), &url)
            .bearer_auth(&self.config.api_key)
            .header("X-Meridian-Client", client_id())
            .header("X-Request-Id", Uuid::new_v4().to_string());
        if let Some(p) = passphrase {
            req = req.header("X-Architecture-Passphrase", p);
        }
        if idempotent {
            req = req.header("Idempotency-Key", Uuid::new_v4().to_string());
        }
        if let Some(v) = if_match {
            req = req.header("If-Match", v.to_string());
        }
        if method != Method::GET {
            if let Some(b) = body {
                req = req.json(&b);
            }
        }
        let response = match req.send().await {
            Ok(r) => r,
            Err(e) => return Reply::Failed(format!("could not reach Meridian: {}", e.without_url())),
        };
        let status = response.status();
        let value: Value = response.json().await.unwrap_or(Value::Null);
        if status.is_success() {
            return Reply::Ok(value);
        }
        let code = value.get("code").and_then(Value::as_str).unwrap_or("");
        let message = value.get("message").and_then(Value::as_str).unwrap_or("request failed");
        match code {
            "PASSPHRASE_REQUIRED" => Reply::PassphraseRequired,
            "PASSPHRASE_INCORRECT" => Reply::PassphraseIncorrect,
            _ => {
                let mut text = format!("Meridian returned {status} {code}: {message}");
                if let Some(secs) = value.get("retryAfterSeconds").and_then(Value::as_u64) {
                    text.push_str(&format!(" (retry after {secs}s)"));
                }
                if let Some(details) = value.get("details").filter(|d| d.as_object().is_some_and(|o| !o.is_empty())) {
                    text.push_str(&format!(" details: {details}"));
                }
                Reply::Failed(text)
            }
        }
    }
}

fn client_id() -> String {
    format!(
        "meridian-mcp/{} ({}/{})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}
