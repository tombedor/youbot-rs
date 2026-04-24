use crate::models::{CaptainLogEntry, CodingAgentProduct, ProjectRecord, SessionState, TaskRecord, TaskStatus};
use crate::task_repository::TaskRepository;
use anyhow::Result;
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct CodingAgentSupervisor {
    task_repository: TaskRepository,
}

impl CodingAgentSupervisor {
    pub fn new(task_repository: TaskRepository) -> Self {
        Self { task_repository }
    }

    pub fn evaluate_background_session(
        &self,
        project: &ProjectRecord,
        task: &TaskRecord,
        product: CodingAgentProduct,
        session_id: &str,
        transcript: &str,
    ) -> Result<SessionState> {
        let lower = transcript.to_ascii_lowercase();
        let state = if lower.contains("waiting for user") || lower.contains("need your input") {
            SessionState::WaitingForInput
        } else if lower.contains("done") || lower.contains("completed") || lower.contains("merged") {
            self.task_repository
                .update_status(project, &task.id, TaskStatus::Complete)?;
            SessionState::Completed
        } else if lower.contains("stuck") || lower.contains("blocked") {
            SessionState::Stuck
        } else {
            SessionState::Active
        };

        let summary = summarize_transcript(transcript);
        self.task_repository
            .append_summary(project, &task.id, product.clone(), session_id, summary)?;

        if matches!(state, SessionState::Completed | SessionState::Stuck) {
            let _ = self.task_repository.load_captains_log(project).map(|mut entries| {
                entries.push(CaptainLogEntry {
                    timestamp: Utc::now(),
                    task_id: task.id.clone(),
                    task_title: task.title.clone(),
                    session_id: session_id.to_string(),
                    product,
                    summary: format!("Session marked {} by supervisor.", state.label()),
                });
                entries
            });
        }

        Ok(state)
    }

    pub fn prompt_for_completion(&self, transcript: &str) -> Option<String> {
        let lower = transcript.to_ascii_lowercase();
        if lower.contains("waiting for user") || lower.contains("need your input") {
            return Some(
                "Continue autonomously if possible. If you are blocked, state the blocker and the next best action."
                    .to_string(),
            );
        }
        None
    }
}

fn summarize_transcript(transcript: &str) -> String {
    let mut lines: Vec<&str> = transcript
        .lines()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .take(4)
        .collect();
    lines.reverse();
    let summary = lines.join(" | ");
    if summary.is_empty() {
        "No transcript captured.".to_string()
    } else {
        summary
    }
}
