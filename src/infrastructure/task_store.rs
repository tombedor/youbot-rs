use crate::domain::{
    AgentSessionRef, CaptainLogEntry, CodingAgentProduct, ProjectRecord, SessionSummary,
    TaskRecord, TaskStatus,
};
use crate::infrastructure::project_catalog::ProjectCatalog;
use crate::infrastructure::state_files;
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const TODO_HEADER: &str = "# TODO\n\n";
const TODO_MARKER_START: &str = "<!-- youbot:tasks ";
const TODO_MARKER_END: &str = " -->";

#[derive(Debug, Clone)]
pub struct TaskStore {
    state_root: PathBuf,
    project_catalog: ProjectCatalog,
}

impl TaskStore {
    pub fn new(state_root: impl Into<PathBuf>, project_catalog: ProjectCatalog) -> Self {
        Self {
            state_root: state_root.into(),
            project_catalog,
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
        self.project_catalog
            .commit_state_snapshot("Update task state")?;
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
        self.project_catalog
            .commit_state_snapshot("Update task state")
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
        self.project_catalog
            .commit_state_snapshot("Update task state")
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
        self.project_catalog
            .commit_state_snapshot("Update task session summary")
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
        self.write_tasks_internal(project, tasks)?;
        self.project_catalog
            .commit_state_snapshot("Update task state")
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

pub fn render_todo_markdown(tasks: &[TaskRecord]) -> String {
    let json = serde_json::to_string_pretty(tasks).expect("task serialization should not fail");
    let mut body = String::from(TODO_HEADER);
    body.push_str(TODO_MARKER_START);
    body.push_str(&json);
    body.push_str(TODO_MARKER_END);
    body.push_str("\n\n");

    if tasks.is_empty() {
        body.push_str("_No tasks yet._\n");
        return body;
    }

    for task in tasks {
        body.push_str(&format!("## {} [{}]\n", task.title, task.status.label()));
        body.push_str(&format!("- id: `{}`\n", task.id));
        body.push_str(&format!("- description: {}\n", task.description));
        if task.sessions.is_empty() {
            body.push_str("- sessions: none\n\n");
            continue;
        }
        for session in &task.sessions {
            body.push_str(&format!(
                "- {} {} session: `{}` ({})\n",
                session.product.label(),
                session.session_kind.label(),
                session.session_id,
                session.state.label()
            ));
            if let Some(branch) = &session.branch_name {
                body.push_str(&format!("  branch: `{branch}`\n"));
            }
            if let Some(summary) = &session.last_summary {
                body.push_str(&format!("  last summary: {}\n", summary.summary));
            }
        }
        body.push('\n');
    }

    body
}

pub fn parse_todo_markdown(body: &str) -> Result<Vec<TaskRecord>> {
    let Some(start) = body.find(TODO_MARKER_START) else {
        return Ok(Vec::new());
    };
    let json_start = start + TODO_MARKER_START.len();
    let remaining = &body[json_start..];
    let Some(end) = remaining.find(TODO_MARKER_END) else {
        return Err(anyhow!("missing TODO metadata terminator"));
    };
    let json = &remaining[..end];
    let tasks = serde_json::from_str(json).context("failed to parse TODO metadata")?;
    Ok(tasks)
}

pub fn render_captains_log(entries: &[CaptainLogEntry]) -> String {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        entries: &'a [CaptainLogEntry],
    }

    let mut body = String::from("# CAPTAINS LOG\n\n");
    body.push_str("<!-- youbot:captains_log ");
    body.push_str(
        &serde_json::to_string_pretty(&Wrapper { entries })
            .expect("captains log serialization should not fail"),
    );
    body.push_str(" -->\n\n");
    for entry in entries.iter().rev() {
        body.push_str(&format!(
            "## {} | {} | {}\n{}\n\n",
            entry.timestamp.to_rfc3339(),
            entry.task_title,
            entry.product.label(),
            entry.summary
        ));
    }
    body
}

pub fn parse_captains_log(body: &str) -> Result<Vec<CaptainLogEntry>> {
    #[derive(Deserialize)]
    struct Wrapper {
        entries: Vec<CaptainLogEntry>,
    }

    let marker = "<!-- youbot:captains_log ";
    let Some(start) = body.find(marker) else {
        return Ok(Vec::new());
    };
    let json_start = start + marker.len();
    let remaining = &body[json_start..];
    let Some(end) = remaining.find(" -->") else {
        return Err(anyhow!("missing captains log metadata terminator"));
    };
    let wrapper: Wrapper =
        serde_json::from_str(&remaining[..end]).context("failed to parse captains log metadata")?;
    Ok(wrapper.entries)
}
