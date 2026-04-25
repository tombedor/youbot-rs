use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub state_root: PathBuf,
    pub managed_repo_root: PathBuf,
    #[serde(default = "default_tmux_socket_name")]
    pub tmux_socket_name: String,
    #[serde(default)]
    pub monitor_silence_seconds: u64,
}

fn default_tmux_socket_name() -> String {
    "youbot".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        let state_root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".youbot");
        Self {
            managed_repo_root: state_root.join("managed_repos"),
            state_root,
            tmux_socket_name: default_tmux_socket_name(),
            monitor_silence_seconds: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub auto_merge: bool,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self { auto_merge: false }
    }
}
