use crate::domain::session::{AgentSessionRef, CodingAgentProduct};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
