use crate::coding_agent_supervisor::CodingAgentSupervisor;
use crate::models::{
    AgentSessionRef, CodingAgentProduct, ProjectRecord, SessionKind, SessionRecord, SessionState,
    TaskRecord,
};
use crate::notifier::Notifier;
use crate::project_registry::ProjectRegistry;
use crate::task_repository::TaskRepository;
use crate::tmux_client::TmuxClient;
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SessionManager {
    state_root: PathBuf,
    monitor_silence_seconds: u64,
    tmux: TmuxClient,
    supervisor: CodingAgentSupervisor,
    notifier: Notifier,
    task_repository: TaskRepository,
    project_registry: ProjectRegistry,
}

impl SessionManager {
    pub fn new(
        state_root: impl Into<PathBuf>,
        monitor_silence_seconds: u64,
        tmux: TmuxClient,
        supervisor: CodingAgentSupervisor,
        notifier: Notifier,
        task_repository: TaskRepository,
        project_registry: ProjectRegistry,
    ) -> Self {
        Self {
            state_root: state_root.into(),
            monitor_silence_seconds,
            tmux,
            supervisor,
            notifier,
            task_repository,
            project_registry,
        }
    }

    pub fn load_sessions(&self) -> Result<Vec<SessionRecord>> {
        let path = self.sessions_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?)
    }

    pub fn start_session(
        &self,
        project: &ProjectRecord,
        task: &TaskRecord,
        product: CodingAgentProduct,
        kind: SessionKind,
    ) -> Result<AgentSessionRef> {
        let mut sessions = self.load_sessions()?;
        if let Some(existing) = sessions.iter().find(|record| {
            record.project_id == project.id
                && record.task_id == task.id
                && record.session.product == product
                && record.session.session_kind == kind
                && self.tmux.session_exists(&record.session.tmux_session_name)
        }) {
            return Ok(existing.session.clone());
        }

        let session_id = Uuid::new_v4().to_string();
        let tmux_session_name = format!("youbot-{}-{}", short_id(&project.id), short_id(&task.id));
        let command = match product {
            CodingAgentProduct::Codex => "codex",
            CodingAgentProduct::ClaudeCode => "claude",
        };

        self.tmux
            .create_session(
                &tmux_session_name,
                &project.path,
                command,
                matches!(kind, SessionKind::Background),
            )
            .with_context(|| format!("failed to start {} session", kind.label()))?;

        if matches!(kind, SessionKind::Background) {
            let _ = self
                .tmux
                .enable_monitor_silence(&tmux_session_name, self.monitor_silence_seconds);
        }

        let session = AgentSessionRef {
            product,
            session_kind: kind,
            tmux_session_name,
            session_id,
            state: SessionState::Active,
            branch_name: None,
            last_summary: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        sessions.push(SessionRecord {
            project_id: project.id.clone(),
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            session: session.clone(),
        });
        self.save_sessions(&sessions)?;
        self.task_repository
            .upsert_session(project, &task.id, session.clone())?;
        Ok(session)
    }

    pub fn attach(&self, session: &AgentSessionRef) -> Result<()> {
        self.tmux.attach(&session.tmux_session_name)
    }

    pub fn finalize_attached_session(
        &self,
        projects: &[ProjectRecord],
        session_name: &str,
    ) -> Result<Option<String>> {
        let mut sessions = self.load_sessions()?;
        let Some(record) = sessions
            .iter_mut()
            .find(|record| record.session.tmux_session_name == session_name)
        else {
            return Ok(None);
        };

        let project = projects
            .iter()
            .find(|project| project.id == record.project_id)
            .ok_or_else(|| anyhow!("unknown project {}", record.project_id))?;
        let tasks = self.task_repository.load_tasks(project)?;
        let task = tasks
            .iter()
            .find(|task| task.id == record.task_id)
            .ok_or_else(|| anyhow!("unknown task {}", record.task_id))?;

        let session_exists = self.tmux.session_exists(session_name);
        let transcript = if session_exists {
            self.tmux.capture_pane(session_name).unwrap_or_default()
        } else {
            String::new()
        };
        let session_id = record.session.session_id.clone();
        let product = record.session.product.clone();
        let status = self.supervisor.evaluate_live_session(
            project,
            task,
            product,
            &session_id,
            &transcript,
        )?;
        record.session.state = if session_exists {
            SessionState::Active
        } else {
            SessionState::Exited
        };
        record.session.updated_at = Utc::now();
        self.task_repository
            .upsert_session(project, &record.task_id, record.session.clone())?;
        self.save_sessions(&sessions)?;

        Ok(Some(format!(
            "Reviewed session {} and set task to {}",
            session_id,
            status.label()
        )))
    }

    pub fn poll(&self, projects: &[ProjectRecord]) -> Result<Vec<SessionRecord>> {
        let mut sessions = self.load_sessions()?;
        for record in &mut sessions {
            if !self.tmux.session_exists(&record.session.tmux_session_name) {
                record.session.state = SessionState::Exited;
                record.session.updated_at = Utc::now();
                continue;
            }

            if !matches!(record.session.session_kind, SessionKind::Background) {
                continue;
            }

            let transcript = self.tmux.capture_pane(&record.session.tmux_session_name)?;
            let project = projects
                .iter()
                .find(|project| project.id == record.project_id)
                .ok_or_else(|| anyhow!("unknown project {}", record.project_id))?;
            let tasks = self.task_repository.load_tasks(project)?;
            let task = tasks
                .iter()
                .find(|task| task.id == record.task_id)
                .ok_or_else(|| anyhow!("unknown task {}", record.task_id))?;
            let state = self.supervisor.evaluate_background_session(
                project,
                task,
                record.session.product.clone(),
                &record.session.session_id,
                &transcript,
            )?;
            record.session.state = state.clone();
            record.session.updated_at = Utc::now();
            self.task_repository.upsert_session(
                project,
                &record.task_id,
                record.session.clone(),
            )?;

            if let Some(prompt) = self.supervisor.prompt_for_completion(&transcript) {
                let _ = self
                    .tmux
                    .send_keys(&record.session.tmux_session_name, &prompt);
            }

            if matches!(state, SessionState::Completed | SessionState::Stuck) {
                let _ = self.notifier.notify(
                    "youbot background session",
                    &format!("{} is {}", record.task_title, state.label()),
                );
            }
        }
        self.save_sessions(&sessions)?;
        Ok(sessions)
    }

    fn save_sessions(&self, sessions: &[SessionRecord]) -> Result<()> {
        let path = self.sessions_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, serde_json::to_string_pretty(sessions)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
        self.project_registry
            .commit_state_snapshot("Update session state")?;
        Ok(())
    }

    fn sessions_path(&self) -> PathBuf {
        self.state_root.join("sessions.json")
    }
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}
