use crate::coding_agent_supervisor::CodingAgentSupervisor;
use crate::models::{
    AgentSessionRef, CodingAgentProduct, ProjectRecord, SessionKind, SessionRecord, SessionState,
    TaskRecord,
};
use crate::notifier::{Notifier, NotifySink};
use crate::project_registry::ProjectRegistry;
use crate::task_repository::TaskRepository;
use crate::tmux_client::{TmuxClient, TmuxOps};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub struct SessionManager {
    state_root: PathBuf,
    monitor_silence_seconds: u64,
    tmux: Arc<dyn TmuxOps>,
    supervisor: CodingAgentSupervisor,
    notifier: Arc<dyn NotifySink>,
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
        Self::with_handles(
            state_root,
            monitor_silence_seconds,
            Arc::new(tmux),
            supervisor,
            Arc::new(notifier),
            task_repository,
            project_registry,
        )
    }

    pub fn with_handles(
        state_root: impl Into<PathBuf>,
        monitor_silence_seconds: u64,
        tmux: Arc<dyn TmuxOps>,
        supervisor: CodingAgentSupervisor,
        notifier: Arc<dyn NotifySink>,
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

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("state_root", &self.state_root)
            .field("monitor_silence_seconds", &self.monitor_silence_seconds)
            .field("supervisor", &self.supervisor)
            .field("task_repository", &self.task_repository)
            .field("project_registry", &self.project_registry)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::SessionManager;
    use crate::coding_agent_supervisor::CodingAgentSupervisor;
    use crate::models::{
        AgentSessionRef, CodingAgentProduct, ProjectConfig, ProjectRecord, SessionKind,
        SessionRecord, SessionState, TaskRecord, TaskStatus,
    };
    use crate::notifier::NotifySink;
    use crate::project_registry::ProjectRegistry;
    use crate::task_repository::TaskRepository;
    use crate::tmux_client::TmuxOps;
    use anyhow::Result;
    use chrono::Utc;
    use std::collections::{HashMap, HashSet};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    #[test]
    fn start_background_session_persists_and_configures_monitoring() {
        let ctx = TestContext::new();
        let task = ctx.create_task("Background work item");

        let session = ctx
            .manager
            .start_session(
                &ctx.project,
                &task,
                CodingAgentProduct::Codex,
                SessionKind::Background,
            )
            .unwrap();

        let sessions = ctx.manager.load_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session.session_kind, SessionKind::Background);
        assert_eq!(
            ctx.tmux_state.lock().unwrap().created[0],
            (
                session.tmux_session_name.clone(),
                ctx.project.path.clone(),
                "codex".to_string(),
                true
            )
        );
        assert_eq!(
            ctx.tmux_state.lock().unwrap().monitor_silence[0],
            (session.tmux_session_name.clone(), 120)
        );
    }

    #[test]
    fn poll_waiting_background_session_prompts_agent() {
        let ctx = TestContext::new();
        let task = ctx.create_task_with_session(
            "Waiting task",
            AgentSessionRef {
                product: CodingAgentProduct::Codex,
                session_kind: SessionKind::Background,
                tmux_session_name: "wait-session".to_string(),
                session_id: "session-1".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        ctx.write_sessions(vec![SessionRecord {
            project_id: ctx.project.id.clone(),
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            session: task.sessions[0].clone(),
        }]);
        {
            let mut tmux = ctx.tmux_state.lock().unwrap();
            tmux.existing.insert("wait-session".to_string());
            tmux.transcripts.insert(
                "wait-session".to_string(),
                "Need your input before I continue".to_string(),
            );
        }

        let sessions = ctx
            .manager
            .poll(std::slice::from_ref(&ctx.project))
            .unwrap();

        assert_eq!(sessions[0].session.state, SessionState::WaitingForInput);
        assert_eq!(ctx.tmux_state.lock().unwrap().sent_keys.len(), 1);
        assert!(
            ctx.tmux_state.lock().unwrap().sent_keys[0]
                .1
                .contains("Continue autonomously if possible")
        );
    }

    #[test]
    fn poll_completed_background_session_updates_task_and_notifies() {
        let ctx = TestContext::new();
        let task = ctx.create_task_with_session(
            "Completed task",
            AgentSessionRef {
                product: CodingAgentProduct::Codex,
                session_kind: SessionKind::Background,
                tmux_session_name: "done-session".to_string(),
                session_id: "session-2".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        ctx.write_sessions(vec![SessionRecord {
            project_id: ctx.project.id.clone(),
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            session: task.sessions[0].clone(),
        }]);
        {
            let mut tmux = ctx.tmux_state.lock().unwrap();
            tmux.existing.insert("done-session".to_string());
            tmux.transcripts.insert(
                "done-session".to_string(),
                "Implemented fix\nCompleted".to_string(),
            );
        }

        let sessions = ctx
            .manager
            .poll(std::slice::from_ref(&ctx.project))
            .unwrap();
        let tasks = ctx.repo.load_tasks(&ctx.project).unwrap();

        assert_eq!(sessions[0].session.state, SessionState::Completed);
        assert_eq!(tasks[0].status, TaskStatus::Complete);
        assert_eq!(ctx.notify_state.lock().unwrap().len(), 1);
    }

    #[test]
    fn finalize_attached_session_marks_exited_and_persists_summary() {
        let ctx = TestContext::new();
        let task = ctx.create_task_with_session(
            "Live task",
            AgentSessionRef {
                product: CodingAgentProduct::Codex,
                session_kind: SessionKind::Live,
                tmux_session_name: "live-session".to_string(),
                session_id: "session-3".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        ctx.write_sessions(vec![SessionRecord {
            project_id: ctx.project.id.clone(),
            task_id: task.id.clone(),
            task_title: task.title.clone(),
            session: task.sessions[0].clone(),
        }]);

        let message = ctx
            .manager
            .finalize_attached_session(std::slice::from_ref(&ctx.project), "live-session")
            .unwrap()
            .unwrap();
        let sessions = ctx.manager.load_sessions().unwrap();

        assert!(message.contains("set task to TODO"));
        assert_eq!(sessions[0].session.state, SessionState::Exited);
        assert!(ctx.repo.load_captains_log(&ctx.project).unwrap().len() >= 1);
    }

    struct TestContext {
        _temp: tempfile::TempDir,
        project: ProjectRecord,
        repo: TaskRepository,
        manager: SessionManager,
        tmux_state: Arc<Mutex<FakeTmuxState>>,
        notify_state: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempdir().unwrap();
            let state_root = temp.path().join(".youbot");
            let registry = ProjectRegistry::new(state_root.clone());
            let repo = TaskRepository::new(state_root.clone(), registry.clone());
            let supervisor = CodingAgentSupervisor::new(repo.clone());
            let tmux_state = Arc::new(Mutex::new(FakeTmuxState::default()));
            let notify_state = Arc::new(Mutex::new(Vec::new()));
            let manager = SessionManager::with_handles(
                state_root.clone(),
                120,
                Arc::new(FakeTmux {
                    state: tmux_state.clone(),
                }),
                supervisor,
                Arc::new(FakeNotifier {
                    sent: notify_state.clone(),
                }),
                repo.clone(),
                registry,
            );
            let project = ProjectRecord {
                id: "project-1".to_string(),
                name: "example".to_string(),
                path: temp.path().join("repo"),
                created_at: Utc::now(),
                config: ProjectConfig::default(),
            };
            std::fs::create_dir_all(&project.path).unwrap();

            Self {
                _temp: temp,
                project,
                repo,
                manager,
                tmux_state,
                notify_state,
            }
        }

        fn create_task(&self, description: &str) -> TaskRecord {
            self.repo
                .create_task(&self.project, "Task title", description)
                .unwrap()
        }

        fn create_task_with_session(
            &self,
            description: &str,
            session: AgentSessionRef,
        ) -> TaskRecord {
            let task = self.create_task(description);
            self.repo
                .upsert_session(&self.project, &task.id, session)
                .unwrap();
            self.repo.load_tasks(&self.project).unwrap().remove(0)
        }

        fn write_sessions(&self, sessions: Vec<SessionRecord>) {
            let sessions_path = self._temp.path().join(".youbot").join("sessions.json");
            std::fs::create_dir_all(sessions_path.parent().unwrap()).unwrap();
            std::fs::write(
                sessions_path,
                serde_json::to_string_pretty(&sessions).unwrap(),
            )
            .unwrap();
        }
    }

    #[derive(Default)]
    struct FakeTmuxState {
        existing: HashSet<String>,
        transcripts: HashMap<String, String>,
        created: Vec<(String, std::path::PathBuf, String, bool)>,
        sent_keys: Vec<(String, String)>,
        monitor_silence: Vec<(String, u64)>,
    }

    struct FakeTmux {
        state: Arc<Mutex<FakeTmuxState>>,
    }

    impl TmuxOps for FakeTmux {
        fn session_exists(&self, session_name: &str) -> bool {
            self.state.lock().unwrap().existing.contains(session_name)
        }

        fn create_session(
            &self,
            session_name: &str,
            cwd: &Path,
            command: &str,
            detached: bool,
        ) -> Result<()> {
            let mut state = self.state.lock().unwrap();
            state.existing.insert(session_name.to_string());
            state.created.push((
                session_name.to_string(),
                cwd.to_path_buf(),
                command.to_string(),
                detached,
            ));
            Ok(())
        }

        fn attach(&self, _session_name: &str) -> Result<()> {
            Ok(())
        }

        fn capture_pane(&self, session_name: &str) -> Result<String> {
            Ok(self
                .state
                .lock()
                .unwrap()
                .transcripts
                .get(session_name)
                .cloned()
                .unwrap_or_default())
        }

        fn send_keys(&self, session_name: &str, input: &str) -> Result<()> {
            self.state
                .lock()
                .unwrap()
                .sent_keys
                .push((session_name.to_string(), input.to_string()));
            Ok(())
        }

        fn enable_monitor_silence(&self, session_name: &str, seconds: u64) -> Result<()> {
            self.state
                .lock()
                .unwrap()
                .monitor_silence
                .push((session_name.to_string(), seconds));
            Ok(())
        }
    }

    struct FakeNotifier {
        sent: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl NotifySink for FakeNotifier {
        fn notify(&self, title: &str, body: &str) -> Result<()> {
            self.sent
                .lock()
                .unwrap()
                .push((title.to_string(), body.to_string()));
            Ok(())
        }
    }
}
