use crate::application::session_review_service::SessionReviewService;
use crate::domain::{
    AgentSessionRef, CodingAgentProduct, ProjectRecord, SessionKind, SessionRecord, SessionState,
    TaskRecord,
};
use crate::infrastructure::notification::{NotificationSink, SystemNotifier};
use crate::infrastructure::project_catalog::ProjectCatalog;
use crate::infrastructure::state_files;
use crate::infrastructure::task_store::TaskStore;
use crate::infrastructure::tmux::{TerminalSessionOps, TmuxTerminal};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionService {
    state_root: PathBuf,
    monitor_silence_seconds: u64,
    tmux: Arc<dyn TerminalSessionOps>,
    session_review_service: SessionReviewService,
    notifier: Arc<dyn NotificationSink>,
    task_store: TaskStore,
    project_catalog: ProjectCatalog,
}

impl SessionService {
    pub fn new(
        state_root: impl Into<PathBuf>,
        monitor_silence_seconds: u64,
        tmux: TmuxTerminal,
        session_review_service: SessionReviewService,
        notifier: SystemNotifier,
        task_store: TaskStore,
        project_catalog: ProjectCatalog,
    ) -> Self {
        Self::with_handles(
            state_root,
            monitor_silence_seconds,
            Arc::new(tmux),
            session_review_service,
            Arc::new(notifier),
            task_store,
            project_catalog,
        )
    }

    pub fn with_handles(
        state_root: impl Into<PathBuf>,
        monitor_silence_seconds: u64,
        tmux: Arc<dyn TerminalSessionOps>,
        session_review_service: SessionReviewService,
        notifier: Arc<dyn NotificationSink>,
        task_store: TaskStore,
        project_catalog: ProjectCatalog,
    ) -> Self {
        Self {
            state_root: state_root.into(),
            monitor_silence_seconds,
            tmux,
            session_review_service,
            notifier,
            task_store,
            project_catalog,
        }
    }

    pub fn load_sessions(&self) -> Result<Vec<SessionRecord>> {
        let path = self.sessions_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        match serde_json::from_str(&raw) {
            Ok(records) => Ok(records),
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
            notification_sent: false,
        });
        self.task_store
            .upsert_session_without_commit(project, &task.id, session.clone())?;
        self.save_sessions_internal(&sessions)?;
        self.project_catalog
            .commit_state_snapshot("Start session")?;
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
        let tasks = self.task_store.load_tasks(project)?;
        let task = tasks
            .iter()
            .find(|task| task.id == record.task_id)
            .ok_or_else(|| anyhow!("unknown task {}", record.task_id))?;

        let session_id = record.session.session_id.clone();
        let session_exists = self.tmux.session_exists(session_name);
        if session_exists {
            let transcript = self.tmux.capture_pane(session_name).unwrap_or_default();
            let product = record.session.product.clone();
            let status = self.session_review_service.evaluate_live_session(
                project,
                task,
                product,
                &session_id,
                &transcript,
            )?;
            record.session.state = SessionState::Active;
            record.session.updated_at = Utc::now();
            self.task_store.upsert_session_without_commit(
                project,
                &record.task_id,
                record.session.clone(),
            )?;
            self.save_sessions_internal(&sessions)?;
            self.project_catalog
                .commit_state_snapshot("Finalize attached session")?;
            return Ok(Some(format!(
                "Reviewed session {} and set task to {}",
                session_id,
                status.label()
            )));
        } else {
            record.session.state = SessionState::Exited;
        }
        record.session.updated_at = Utc::now();
        record.notification_sent = false;
        self.task_store.upsert_session_without_commit(
            project,
            &record.task_id,
            record.session.clone(),
        )?;
        self.save_sessions_internal(&sessions)?;
        self.project_catalog
            .commit_state_snapshot("Finalize attached session")?;

        Ok(Some(format!(
            "Session {} exited before transcript could be captured",
            session_id
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
            let tasks = self.task_store.load_tasks(project)?;
            let task = tasks
                .iter()
                .find(|task| task.id == record.task_id)
                .ok_or_else(|| anyhow!("unknown task {}", record.task_id))?;
            let state = self.session_review_service.evaluate_background_session(
                project,
                task,
                record.session.product.clone(),
                &record.session.session_id,
                &transcript,
            )?;
            record.session.state = state.clone();
            record.session.updated_at = Utc::now();
            self.task_store.upsert_session_without_commit(
                project,
                &record.task_id,
                record.session.clone(),
            )?;

            if let Some(prompt) = self
                .session_review_service
                .prompt_for_completion(&transcript)
            {
                let _ = self
                    .tmux
                    .send_keys(&record.session.tmux_session_name, &prompt);
            }

            if matches!(state, SessionState::Completed | SessionState::Stuck) {
                if !record.notification_sent {
                    let _ = self.notifier.notify(
                        "youbot background session",
                        &format!("{} is {}", record.task_title, state.label()),
                    );
                    record.notification_sent = true;
                }
            } else {
                record.notification_sent = false;
            }
        }
        self.save_sessions_internal(&sessions)?;
        self.project_catalog
            .commit_state_snapshot("Poll sessions")?;
        Ok(sessions)
    }

    fn save_sessions_internal(&self, sessions: &[SessionRecord]) -> Result<()> {
        let path = self.sessions_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        state_files::atomic_write(&path, serde_json::to_string_pretty(sessions)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    fn sessions_path(&self) -> PathBuf {
        self.state_root.join("sessions.json")
    }
}

fn short_id(id: &str) -> &str {
    &id[..id.len().min(8)]
}

impl std::fmt::Debug for SessionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionService")
            .field("state_root", &self.state_root)
            .field("monitor_silence_seconds", &self.monitor_silence_seconds)
            .field("session_review_service", &self.session_review_service)
            .field("task_store", &self.task_store)
            .field("project_catalog", &self.project_catalog)
            .finish()
    }
}
