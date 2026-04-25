use crate::application::agent_policy;
use crate::domain::{
    AgentSessionRef, CodingAgentProduct, ProjectRecord, SessionKind, TaskRecord, TaskStatus,
};
use crate::infrastructure::state_history::StateHistory;
use crate::infrastructure::task_store::TaskStore;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct TaskService {
    task_store: TaskStore,
    state_history: StateHistory,
}

impl TaskService {
    pub fn new(task_store: TaskStore, state_history: StateHistory) -> Self {
        Self {
            task_store,
            state_history,
        }
    }

    pub fn load_tasks(&self, project: &ProjectRecord) -> Result<Vec<TaskRecord>> {
        self.task_store.load_tasks(project)
    }

    pub fn create_task(
        &self,
        project: &ProjectRecord,
        description: impl Into<String>,
    ) -> Result<TaskRecord> {
        let description = description.into();
        let title = agent_policy::classify_task_title(&description);
        let task = self.task_store.create_task(project, title, description)?;
        self.state_history.commit_snapshot("Update task state")?;
        Ok(task)
    }

    pub fn set_status(
        &self,
        project: &ProjectRecord,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<()> {
        self.task_store.update_status(project, task_id, status)?;
        self.state_history.commit_snapshot("Update task state")
    }

    pub fn find_session(
        &self,
        task: &TaskRecord,
        product: CodingAgentProduct,
        kind: SessionKind,
    ) -> Result<AgentSessionRef> {
        let session = task
            .sessions
            .iter()
            .find(|session| session.product == product && session.session_kind == kind)
            .ok_or_else(|| anyhow!("No {} {} session to attach", product.label(), kind.label()))?;
        Ok(session.clone())
    }
}
