//! Thin HTTPS client for `meridian-backend`. The binary is stateless: the backend owns the
//! model, the gap rules, entitlements, and payments (`ADR-AppArch-25`).
//!
//! Wire contract: `meridian-docs/architecture/mcp-api.md` (v1).

use anyhow::{Context, Result};
use std::sync::Mutex;
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
    /// The API key: from the environment, or obtained at runtime by `meridian_setup`. Memory only.
    api_key: Mutex<Option<String>>,
}

/// A reply from an endpoint outside the MCP API (sign-up, login, API key): status and body text.
pub struct Raw {
    pub status: u16,
    pub text: String,
}

impl Raw {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or(Value::Null)
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

impl Backend {
    pub fn new(config: Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(client_id())
            .build()
            .context("building HTTP client")?;
        let api_key = Mutex::new(config.api_key.clone());
        Ok(Self { http, config, api_key })
    }

    pub fn set_api_key(&self, key: String) {
        *self.api_key.lock().unwrap_or_else(|e| e.into_inner()) = Some(key);
    }

    fn api_key(&self) -> Option<String> {
        self.api_key.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Call an endpoint under the backend's `/api` root that is not part of the MCP API, with an optional bearer
    /// credential. Used only by `meridian_setup`; the body and the reply are never logged.
    pub async fn raw(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        bearer: Option<&str>,
        headers: &[(&str, &str)],
    ) -> Result<Raw, String> {
        let url = format!("{}{}", self.config.api_url.trim_end_matches('/'), path);
        let mut req = self
            .http
            .request(method, &url)
            .header("X-Meridian-Client", client_id())
            .header("X-Request-Id", Uuid::new_v4().to_string());
        if let Some(b) = bearer {
            req = req.bearer_auth(b);
        }
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let response = req.send().await.map_err(|e| format!("could not reach Meridian: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        Ok(Raw { status, text })
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
        let Some(api_key) = self.api_key() else {
            return Reply::Failed(
                "No Meridian API key is set. Call `meridian_setup` to create an account and get one, or set \
                 MERIDIAN_API_KEY."
                    .to_string(),
            );
        };
        let mut req = self
            .http
            .request(method.clone(), &url)
            .bearer_auth(&api_key)
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
