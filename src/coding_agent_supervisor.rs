use crate::models::{
    CaptainLogEntry, CodingAgentProduct, ProjectRecord, SessionState, TaskRecord, TaskStatus,
};
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
        } else if lower.contains("done") || lower.contains("completed") || lower.contains("merged")
        {
            self.task_repository
                .update_status(project, &task.id, TaskStatus::Complete)?;
            SessionState::Completed
        } else if lower.contains("stuck") || lower.contains("blocked") {
            SessionState::Stuck
        } else {
            SessionState::Active
        };

        let summary = summarize_transcript(transcript);
        self.task_repository.append_summary(
            project,
            &task.id,
            product.clone(),
            session_id,
            summary,
        )?;

        if matches!(state, SessionState::Completed | SessionState::Stuck) {
            let _ = self
                .task_repository
                .load_captains_log(project)
                .map(|mut entries| {
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

    pub fn evaluate_live_session(
        &self,
        project: &ProjectRecord,
        task: &TaskRecord,
        product: CodingAgentProduct,
        session_id: &str,
        transcript: &str,
    ) -> Result<TaskStatus> {
        let status = infer_task_status(transcript).unwrap_or_else(|| task.status.clone());
        self.task_repository
            .update_status(project, &task.id, status.clone())?;
        self.task_repository.append_summary(
            project,
            &task.id,
            product,
            session_id,
            summarize_transcript(transcript),
        )?;
        Ok(status)
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

    pub fn classify_task_title(&self, description: &str) -> String {
        let compact = description.split_whitespace().collect::<Vec<_>>().join(" ");
        if compact.is_empty() {
            return "Untitled task".to_string();
        }

        let cleaned = compact
            .trim_end_matches(['.', '!', '?', ';', ':'])
            .to_string();
        let mut words = cleaned.split_whitespace();
        let title = words.by_ref().take(7).collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            "Untitled task".to_string()
        } else if words.next().is_some() {
            format!("{title}...")
        } else {
            title
        }
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

fn infer_task_status(transcript: &str) -> Option<TaskStatus> {
    let lower = transcript.to_ascii_lowercase();
    if lower.contains("wont do") || lower.contains("won't do") {
        Some(TaskStatus::WontDo)
    } else if lower.contains("done")
        || lower.contains("completed")
        || lower.contains("merged")
        || lower.contains("fixed")
    {
        Some(TaskStatus::Complete)
    } else if lower.contains("stuck") || lower.contains("blocked") {
        Some(TaskStatus::InProgress)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::CodingAgentSupervisor;
    use crate::models::{
        AgentSessionRef, CodingAgentProduct, ProjectConfig, ProjectRecord, SessionKind,
        SessionState, TaskRecord, TaskStatus,
    };
    use crate::project_registry::ProjectRegistry;
    use crate::task_repository::TaskRepository;
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn classify_task_title_trims_and_shortens_description() {
        let (_temp, project, repo, supervisor) = test_context();
        let _ = (project, repo); // keep setup symmetrical for future tests

        let title =
            supervisor.classify_task_title("  Implement the long-running background poller now. ");

        assert_eq!(title, "Implement the long-running background poller now");
    }

    #[test]
    fn prompt_for_completion_only_when_waiting_for_input() {
        let (_temp, _project, _repo, supervisor) = test_context();

        assert!(
            supervisor
                .prompt_for_completion("Agent is waiting for user input")
                .is_some()
        );
        assert!(supervisor.prompt_for_completion("Still coding").is_none());
    }

    #[test]
    fn evaluate_live_session_updates_status_and_summary() {
        let (_temp, project, repo, supervisor) = test_context();
        let task = create_task_with_session(
            &repo,
            &project,
            "Fix failing integration workflow",
            CodingAgentProduct::Codex,
            SessionKind::Live,
        );

        let status = supervisor
            .evaluate_live_session(
                &project,
                &task,
                CodingAgentProduct::Codex,
                "session-1",
                "Investigated issue\nApplied fix\nDone",
            )
            .unwrap();

        let stored = repo.load_tasks(&project).unwrap();
        assert_eq!(status, TaskStatus::Complete);
        assert_eq!(stored[0].status, TaskStatus::Complete);
        assert_eq!(
            stored[0].sessions[0].last_summary.as_ref().unwrap().summary,
            "Investigated issue | Applied fix | Done"
        );
    }

    #[test]
    fn evaluate_background_session_marks_waiting_and_completed_states() {
        let (_temp, project, repo, supervisor) = test_context();
        let task = create_task_with_session(
            &repo,
            &project,
            "Make background sessions autonomous",
            CodingAgentProduct::Codex,
            SessionKind::Background,
        );

        let waiting = supervisor
            .evaluate_background_session(
                &project,
                &task,
                CodingAgentProduct::Codex,
                "session-1",
                "I need your input before I can continue",
            )
            .unwrap();
        let completed = supervisor
            .evaluate_background_session(
                &project,
                &task,
                CodingAgentProduct::Codex,
                "session-1",
                "Applied the patch\nMerged the changes\nCompleted",
            )
            .unwrap();

        let stored = repo.load_tasks(&project).unwrap();
        assert_eq!(waiting, SessionState::WaitingForInput);
        assert_eq!(completed, SessionState::Completed);
        assert_eq!(stored[0].status, TaskStatus::Complete);
        assert!(repo.load_captains_log(&project).unwrap().len() >= 2);
    }

    fn test_context() -> (
        tempfile::TempDir,
        ProjectRecord,
        TaskRepository,
        CodingAgentSupervisor,
    ) {
        let temp = tempdir().unwrap();
        let state_root = temp.path().join(".youbot");
        let registry = ProjectRegistry::new(state_root.clone());
        let repo = TaskRepository::new(state_root.clone(), registry);
        let supervisor = CodingAgentSupervisor::new(repo.clone());
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "example".to_string(),
            path: temp.path().join("repo"),
            created_at: Utc::now(),
            config: ProjectConfig::default(),
        };
        std::fs::create_dir_all(&project.path).unwrap();
        (temp, project, repo, supervisor)
    }

    fn create_task_with_session(
        repo: &TaskRepository,
        project: &ProjectRecord,
        description: &str,
        product: CodingAgentProduct,
        kind: SessionKind,
    ) -> TaskRecord {
        let task = repo
            .create_task(project, "Task title", description)
            .unwrap();
        repo.upsert_session(
            project,
            &task.id,
            AgentSessionRef {
                product,
                session_kind: kind,
                tmux_session_name: "tmux-1".to_string(),
                session_id: "session-1".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        )
        .unwrap();
        repo.load_tasks(project).unwrap().remove(0)
    }
}
