use crate::domain::{
    AgentSessionRef, CaptainLogEntry, CodingAgentProduct, ProjectRecord, SessionSummary,
    TaskRecord, TaskStatus,
};
use crate::infrastructure::captains_log_format::{parse_captains_log, render_captains_log};
use crate::infrastructure::state_files;
use crate::infrastructure::todo_format::{parse_todo_markdown, render_todo_markdown};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TaskStore {
    state_root: PathBuf,
}

impl TaskStore {
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
        }
    }

    pub fn load_tasks(&self, project: &ProjectRecord) -> Result<Vec<TaskRecord>> {
        let path = self.todo_path(project);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        match parse_todo_markdown(&raw) {
            Ok(tasks) => Ok(tasks),
            Err(error) => {
                let quarantine_path = state_files::quarantine_corrupt(&path)?;
                eprintln!(
                    "warning: failed to parse {}; moved corrupt file to {}: {error}",
                    path.display(),
                    quarantine_path.display()
                );
                Ok(Vec::new())
            }
        }
    }

    pub fn create_task(
        &self,
        project: &ProjectRecord,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<TaskRecord> {
        let mut tasks = self.load_tasks(project)?;
        let task = TaskRecord {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            description: description.into(),
            status: TaskStatus::Todo,
            sessions: Vec::new(),
        };
        tasks.push(task.clone());
        self.write_tasks_internal(project, &tasks)?;
        Ok(task)
    }

    pub fn update_status(
        &self,
        project: &ProjectRecord,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<()> {
        let mut tasks = self.load_tasks(project)?;
        let task = tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| anyhow!("unknown task id {task_id}"))?;
        task.status = status;
        self.write_tasks_internal(project, &tasks)?;
        Ok(())
    }

    pub fn upsert_session(
        &self,
        project: &ProjectRecord,
        task_id: &str,
        session: AgentSessionRef,
    ) -> Result<()> {
        let mut tasks = self.load_tasks(project)?;
        self.upsert_session_in_tasks(&mut tasks, task_id, session)?;
        self.write_tasks_internal(project, &tasks)?;
        Ok(())
    }

    pub fn append_summary(
        &self,
        project: &ProjectRecord,
        task_id: &str,
        product: CodingAgentProduct,
        session_id: &str,
        summary: impl Into<String>,
    ) -> Result<()> {
        let summary = summary.into();
        let mut tasks = self.load_tasks(project)?;
        let entry = {
            let task = tasks
                .iter_mut()
                .find(|task| task.id == task_id)
                .ok_or_else(|| anyhow!("unknown task id {task_id}"))?;

            let session = task
                .sessions
                .iter_mut()
                .find(|session| session.session_id == session_id)
                .ok_or_else(|| anyhow!("no session for {}", product.label()))?;
            session.last_summary = Some(SessionSummary {
                summary: summary.clone(),
                updated_at: Utc::now(),
            });
            session.updated_at = Utc::now();
            CaptainLogEntry {
                timestamp: Utc::now(),
                task_id: task.id.clone(),
                task_title: task.title.clone(),
                session_id: session_id.to_string(),
                product,
                summary,
            }
        };
        self.write_tasks_internal(project, &tasks)?;
        let mut entries = self.load_captains_log(project)?;
        entries.push(entry);
        self.write_captains_log_internal(project, &entries)?;
        Ok(())
    }

    pub fn load_captains_log(&self, project: &ProjectRecord) -> Result<Vec<CaptainLogEntry>> {
        let path = self.captains_log_path(project);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        match parse_captains_log(&raw) {
            Ok(entries) => Ok(entries),
            Err(error) => {
                let quarantine_path = state_files::quarantine_corrupt(&path)?;
                eprintln!(
                    "warning: failed to parse {}; moved corrupt file to {}: {error}",
                    path.display(),
                    quarantine_path.display()
                );
                Ok(Vec::new())
            }
        }
    }

    pub fn write_tasks(&self, project: &ProjectRecord, tasks: &[TaskRecord]) -> Result<()> {
        self.write_tasks_internal(project, tasks)
    }

    pub fn upsert_session_without_commit(
        &self,
        project: &ProjectRecord,
        task_id: &str,
        session: AgentSessionRef,
    ) -> Result<()> {
        let mut tasks = self.load_tasks(project)?;
        self.upsert_session_in_tasks(&mut tasks, task_id, session)?;
        self.write_tasks_internal(project, &tasks)
    }

    fn write_tasks_internal(&self, project: &ProjectRecord, tasks: &[TaskRecord]) -> Result<()> {
        let project_dir = self.project_state_dir(project);
        fs::create_dir_all(&project_dir)
            .with_context(|| format!("failed to create {}", project_dir.display()))?;
        let path = self.todo_path(project);
        state_files::atomic_write(&path, render_todo_markdown(tasks))
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    fn write_captains_log_internal(
        &self,
        project: &ProjectRecord,
        entries: &[CaptainLogEntry],
    ) -> Result<()> {
        let path = self.captains_log_path(project);
        state_files::atomic_write(&path, render_captains_log(entries))
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    fn upsert_session_in_tasks(
        &self,
        tasks: &mut [TaskRecord],
        task_id: &str,
        session: AgentSessionRef,
    ) -> Result<()> {
        let task = tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or_else(|| anyhow!("unknown task id {task_id}"))?;

        if let Some(existing) = task.sessions.iter_mut().find(|existing| {
            existing.product == session.product && existing.session_kind == session.session_kind
        }) {
            *existing = session;
        } else {
            task.sessions.push(session);
        }
        Ok(())
    }

    fn project_state_dir(&self, project: &ProjectRecord) -> PathBuf {
        self.state_root.join("projects").join(&project.id)
    }

    fn todo_path(&self, project: &ProjectRecord) -> PathBuf {
        self.project_state_dir(project).join("TODO.md")
    }

    fn captains_log_path(&self, project: &ProjectRecord) -> PathBuf {
        self.project_state_dir(project).join("CAPTAINS_LOG.md")
    }
}
