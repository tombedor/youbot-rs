use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub repos: Vec<RepoConfig>,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub coding_agent: CodingAgentConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub repo_id: String,
    pub name: String,
    pub path: PathBuf,
    pub classification: RepoClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepoClassification {
    Integrated,
    Managed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub jobs: Vec<SchedulerJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerJob {
    pub job_id: String,
    pub repo_id: String,
    pub command_name: String,
    pub schedule_type: String,
    #[serde(default)]
    pub cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentConfig {
    pub default_backend: String,
    #[serde(default)]
    pub backends: serde_json::Map<String, serde_json::Value>,
}

impl Default for CodingAgentConfig {
    fn default() -> Self {
        let mut backends = serde_json::Map::new();
        backends.insert(
            "codex".to_string(),
            serde_json::json!({"command_prefix": ["codex"], "default_args": []}),
        );
        backends.insert(
            "claude_code".to_string(),
            serde_json::json!({"command_prefix": ["claude"], "default_args": []}),
        );
        Self {
            default_backend: "codex".to_string(),
            backends,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub last_active_repo_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    pub repo_id: String,
    pub name: String,
    pub path: PathBuf,
    pub classification: RepoClassification,
    pub status: RepoStatus,
    #[serde(default)]
    pub purpose_summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub preferred_commands: Vec<String>,
    #[serde(default)]
    pub commands: Vec<CommandRecord>,
    #[serde(default)]
    pub last_scanned_at: Option<String>,
    #[serde(default)]
    pub last_active_at: Option<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub preferred_backend: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepoStatus {
    Ready,
    Invalid,
    Missing,
    Error,
}

impl RepoStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Invalid => "invalid",
            Self::Missing => "missing",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRecord {
    pub repo_id: String,
    pub command_name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub invocation: Vec<String>,
    #[serde(default)]
    pub supports_structured_output: bool,
    pub structured_output_format: StructuredOutputFormat,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StructuredOutputFormat {
    Json,
    Text,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub conversation_id: String,
    #[serde(default)]
    pub messages: Vec<ConversationMessage>,
    pub updated_at: String,
    #[serde(default)]
    pub last_response_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub message_id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl MessageRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub repo_id: String,
    pub command_name: String,
    pub invocation: Vec<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub started_at: String,
    pub finished_at: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAction {
    Command,
    CodeChange,
    AdapterChange,
    Clarify,
}

#[derive(Debug, Clone)]
pub struct RouteDecision {
    pub action: RouteAction,
    pub repo_index: usize,
    pub command_name: String,
    pub args: Vec<String>,
    pub reasoning: String,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RepoOverview {
    pub subtitle: String,
    pub cards: Vec<OverviewCard>,
    pub quick_actions: Vec<QuickActionView>,
}

#[derive(Debug, Clone)]
pub struct OverviewCard {
    pub title: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct QuickActionView {
    pub title: String,
    pub command_name: String,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentSessionRef {
    pub repo_id: String,
    pub backend_name: String,
    pub session_kind: String,
    pub session_id: String,
    #[serde(default)]
    pub purpose_summary: Option<String>,
    pub status: String,
    pub last_used_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterRecord {
    pub adapter_id: String,
    pub repo_id: String,
    pub version: String,
    #[serde(default)]
    pub view_names: Vec<String>,
    #[serde(default)]
    pub command_palette_entries: Vec<String>,
    #[serde(default)]
    pub output_rules: Vec<String>,
    pub updated_at: String,
    #[serde(default)]
    pub overview_sections: Vec<OverviewSectionSpec>,
    #[serde(default)]
    pub quick_actions: Vec<QuickActionSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewSectionSpec {
    pub command_name: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub max_lines: usize,
    #[serde(default)]
    pub fallback_command_names: Vec<String>,
    pub render_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickActionSpec {
    pub command_name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentRunLog {
    pub repo_id: String,
    pub backend_name: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub request_summary: Option<String>,
    pub exit_code: i32,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRunLog {
    pub repo_id: String,
    pub command_name: String,
    pub invocation: Vec<String>,
    pub exit_code: i32,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingAgentActivity {
    pub run_id: String,
    pub target_repo_id: String,
    pub target_kind: String,
    pub backend_name: String,
    pub request_summary: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub recent_entries: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewBundle {
    pub bundle_id: String,
    pub created_at: String,
    pub source_state_root: String,
    pub window_summary: String,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<ConversationMessage>,
    #[serde(default)]
    pub command_runs: Vec<serde_json::Value>,
    #[serde(default)]
    pub coding_agent_runs: Vec<serde_json::Value>,
    #[serde(default)]
    pub activity_entries: Vec<serde_json::Value>,
    #[serde(default)]
    pub activity_log_refs: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}
