use crate::config::state_root;
use crate::models::{ConversationMessage, ConversationRecord, MessageRole};
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;

pub struct ConversationStore {
    path: PathBuf,
    record: ConversationRecord,
}

impl ConversationStore {
    pub fn load_or_create() -> Result<Self> {
        let path = state_root()?.join("conversation").join("history.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let record = if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            serde_json::from_str(&raw)
                .with_context(|| format!("failed to parse {}", path.display()))?
        } else {
            let record = ConversationRecord {
                conversation_id: format!("conversation-{}", Utc::now().timestamp()),
                messages: Vec::new(),
                updated_at: now_string(),
                last_response_id: None,
            };
            let body = serde_json::to_string_pretty(&record)
                .context("failed to serialize conversation")?;
            fs::write(&path, body)
                .with_context(|| format!("failed to write {}", path.display()))?;
            record
        };

        Ok(Self { path, record })
    }

    pub fn record(&self) -> &ConversationRecord {
        &self.record
    }

    pub fn append(&mut self, role: MessageRole, content: impl Into<String>) -> Result<()> {
        self.record.messages.push(ConversationMessage {
            message_id: format!("msg-{}", Utc::now().timestamp_micros()),
            role,
            content: content.into(),
            created_at: now_string(),
        });
        self.record.updated_at = now_string();
        self.save()
    }

    pub fn set_last_response_id(&mut self, response_id: impl Into<Option<String>>) -> Result<()> {
        self.record.last_response_id = response_id.into();
        self.record.updated_at = now_string();
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        let body = serde_json::to_string_pretty(&self.record)
            .context("failed to serialize conversation")?;
        fs::write(&self.path, body)
            .with_context(|| format!("failed to write {}", self.path.display()))?;
        Ok(())
    }

    #[cfg(test)]
    pub fn from_record(record: ConversationRecord) -> Self {
        let path = std::env::temp_dir().join(format!(
            "youbot-rs-test-conversation-{}.json",
            std::process::id()
        ));
        Self { path, record }
    }
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}
