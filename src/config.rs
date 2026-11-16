//! Runtime configuration, read from the environment.

use anyhow::{Context, Result};

/// Where the backend lives and how to authenticate to it.
#[derive(Clone)]
pub struct Config {
    pub api_url: String,
    pub api_key: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let api_url = std::env::var("MERIDIAN_API_URL")
            .unwrap_or_else(|_| "https://resolvingarchitecture.io/meridian/api".to_string());
        let api_key = std::env::var("MERIDIAN_API_KEY")
            .context("MERIDIAN_API_KEY is not set; create an API key in meridian-web")?;
        Ok(Self { api_url, api_key })
    }
}
