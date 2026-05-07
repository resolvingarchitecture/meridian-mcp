use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MeridianConfig {
    pub api_key: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("meridian");

    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config directory: {}", dir.display()))?;

    Ok(dir.join("config.json"))
}

pub fn load() -> Result<MeridianConfig> {
    let path = config_path()?;

    if !path.exists() {
        return Ok(MeridianConfig::default());
    }

    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;

    let config = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;

    Ok(config)
}

pub fn set_api_key(api_key: &str) -> Result<()> {
    let mut config = load().unwrap_or_default();
    config.api_key = Some(api_key.to_string());

    let path = config_path()?;
    let bytes = serde_json::to_vec_pretty(&config)?;

    fs::write(&path, bytes)
        .with_context(|| format!("failed to write config file: {}", path.display()))?;

    Ok(())
}

pub fn api_key() -> Result<String> {
    if let Ok(key) = std::env::var("MERIDIAN_API_KEY") {
        if !key.trim().is_empty() {
            return Ok(key);
        }
    }

    let config = load()?;

    config
        .api_key
        .filter(|key| !key.trim().is_empty())
        .context("MERIDIAN_API_KEY not set and no API key configured. Run: meridian config set api-key <key>")
}

pub fn config_file_display_path() -> String {
    config_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "<unavailable>".to_string())
}