//! Runtime configuration, read from the environment.

use anyhow::Result;

/// Where the backend lives and how to authenticate to it.
#[derive(Clone)]
pub struct Config {
    pub api_url: String,
    /// Absent until the user has an account; `meridian_setup` can obtain one at runtime.
    pub api_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let api_url = std::env::var("MERIDIAN_API_URL")
            .unwrap_or_else(|_| "https://resolvingarchitecture.io/apps/meridian/api".to_string());
        let api_key = std::env::var("MERIDIAN_API_KEY").ok().filter(|k| !k.trim().is_empty());
        Ok(Self { api_url, api_key })
    }
}
