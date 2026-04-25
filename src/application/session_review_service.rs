use crate::application::agent_policy;
use crate::domain::{CodingAgentProduct, ProjectRecord, SessionState, TaskRecord, TaskStatus};
use crate::infrastructure::task_store::TaskStore;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SessionReviewService {
    task_store: TaskStore,
}

impl SessionReviewService {
    pub fn new(task_store: TaskStore) -> Self {
        Self { task_store }
    }

    pub fn evaluate_background_session(
        &self,
        project: &ProjectRecord,
        task: &TaskRecord,
        product: CodingAgentProduct,
        session_id: &str,
        transcript: &str,
    ) -> Result<SessionState> {
        let state = agent_policy::infer_background_state(transcript);
        if matches!(state, SessionState::Completed) {
            self.task_store
                .update_status(project, &task.id, TaskStatus::Complete)?;
        }

        let summary = agent_policy::summarize_transcript(transcript);
        self.task_store
            .append_summary(project, &task.id, product.clone(), session_id, summary)?;

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
        let status =
            agent_policy::infer_task_status(transcript).unwrap_or_else(|| task.status.clone());
        self.task_store
            .update_status(project, &task.id, status.clone())?;
        self.task_store.append_summary(
            project,
            &task.id,
            product,
            session_id,
            agent_policy::summarize_transcript(transcript),
        )?;
        Ok(status)
    }

    pub fn prompt_for_completion(&self, transcript: &str) -> Option<String> {
        agent_policy::prompt_for_completion(transcript)
    }

    pub fn classify_task_title(&self, description: &str) -> String {
        agent_policy::classify_task_title(description)
    }
}
