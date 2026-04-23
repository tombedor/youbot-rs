use crate::models::AppConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub fn state_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to determine home directory")?;
    Ok(home.join(".youbot"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(state_root()?.join("config.json"))
}

pub fn load_or_create() -> Result<AppConfig> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir {}", parent.display()))?;
    }

    if !path.exists() {
        let config = AppConfig::default();
        save(&config)?;
        return Ok(config);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let config = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse config {}", path.display()))?;
    Ok(config)
}

pub fn save(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create state dir {}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(config).context("failed to serialize config")?;
    fs::write(&path, body).with_context(|| format!("failed to write config {}", path.display()))?;
    Ok(())
}
