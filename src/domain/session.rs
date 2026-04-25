use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
pub struct SessionRecord {
    pub project_id: String,
    pub task_id: String,
    pub task_title: String,
    pub session: AgentSessionRef,
    #[serde(default)]
    pub notification_sent: bool,
}
