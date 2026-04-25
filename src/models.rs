use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectRecord {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub created_at: DateTime<Utc>,
    pub config: ProjectConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Complete,
    WontDo,
}

impl TaskStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::InProgress => "IN PROGRESS",
            Self::Complete => "COMPLETE",
            Self::WontDo => "WONT DO",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CodingAgentProduct {
    Codex,
    ClaudeCode,
}

impl CodingAgentProduct {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionKind {
    Live,
    Background,
}

impl SessionKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Background => "background",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    Active,
    WaitingForInput,
    Completed,
    Stuck,
    Exited,
}

impl SessionState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::WaitingForInput => "waiting",
            Self::Completed => "completed",
            Self::Stuck => "stuck",
            Self::Exited => "exited",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub summary: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionRef {
    pub product: CodingAgentProduct,
    pub session_kind: SessionKind,
    pub tmux_session_name: String,
    pub session_id: String,
    pub state: SessionState,
    pub branch_name: Option<String>,
    pub last_summary: Option<SessionSummary>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub sessions: Vec<AgentSessionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptainLogEntry {
    pub timestamp: DateTime<Utc>,
    pub task_id: String,
    pub task_title: String,
    pub session_id: String,
    pub product: CodingAgentProduct,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub project_id: String,
    pub task_id: String,
    pub task_title: String,
    pub session: AgentSessionRef,
}

#[derive(Debug, Clone, Default)]
pub struct AddRepoForm {
    pub repo_path: String,
    pub create_new_repo: bool,
    pub programming_language: String,
    pub create_location_policy: usize,
    pub remote_mode: usize,
    pub dont_ask_again: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    ProjectDetail,
    TaskDetail,
    AddRepo,
    LiveSession,
}
