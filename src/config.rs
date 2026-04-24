use crate::models::AppConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn state_root() -> Result<PathBuf> {
    Ok(load_or_create()?.state_root)
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join("config.json")
}

pub fn load_or_create() -> Result<AppConfig> {
    let mut config = AppConfig::default();
    fs::create_dir_all(&config.state_root)
        .with_context(|| format!("failed to create {}", config.state_root.display()))?;

    let path = config_path(&config.state_root);
    if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        config = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
    } else {
        save(&config)?;
    }

    fs::create_dir_all(&config.managed_repo_root)
        .with_context(|| format!("failed to create {}", config.managed_repo_root.display()))?;
    fs::create_dir_all(config.state_root.join("projects"))
        .with_context(|| format!("failed to create {}", config.state_root.join("projects").display()))?;
    Ok(config)
}

pub fn save(config: &AppConfig) -> Result<()> {
    fs::create_dir_all(&config.state_root)
        .with_context(|| format!("failed to create {}", config.state_root.display()))?;
    let body = serde_json::to_string_pretty(config)?;
    let path = config_path(&config.state_root);
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}
