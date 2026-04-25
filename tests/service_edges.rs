use anyhow::Result;
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use youbot_rs::application::project_service::ProjectService;
use youbot_rs::application::session_review_service::SessionReviewService;
use youbot_rs::application::session_service::SessionService;
use youbot_rs::application::task_service::TaskService;
use youbot_rs::domain::{
    AgentSessionRef, CodingAgentProduct, ProjectConfig, ProjectRecord, SessionKind, SessionRecord,
    SessionState, TaskRecord, TaskStatus,
};
use youbot_rs::infrastructure::notification::NotificationSink;
use youbot_rs::infrastructure::project_catalog::ProjectCatalog;
use youbot_rs::infrastructure::state_files;
use youbot_rs::infrastructure::state_history::StateHistory;
use youbot_rs::infrastructure::task_store::TaskStore;
use youbot_rs::infrastructure::tmux::TerminalSessionOps;

#[test]
fn project_service_rejects_duplicate_repo_registration() {
    let temp = tempdir().unwrap();
    let state_root = temp.path().join(".youbot");
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();

    let service = ProjectService::new(
        ProjectCatalog::new(state_root.clone()),
        StateHistory::new(state_root),
    );

    service.add_existing_repo(&repo_path, false).unwrap();
    let error = service.add_existing_repo(&repo_path, false).unwrap_err();

    assert!(error.to_string().contains("repo already registered"));
}

#[test]
fn task_service_find_session_returns_error_for_missing_session() {
    let temp = tempdir().unwrap();
    let state_root = temp.path().join(".youbot");
    let service = TaskService::new(
        TaskStore::new(state_root.clone()),
        StateHistory::new(state_root),
    );
    let task = TaskRecord {
        id: "task-1".to_string(),
        title: "Task".to_string(),
        description: "desc".to_string(),
        status: TaskStatus::Todo,
        sessions: Vec::new(),
    };

    let error = service
        .find_session(&task, CodingAgentProduct::Codex, SessionKind::Background)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("No codex background session to attach")
    );
}

#[test]
fn session_service_reuses_existing_active_session() {
    let ctx = ServiceContext::new();
    let task = ctx.create_task("Reuse existing session");
    let session = AgentSessionRef {
        product: CodingAgentProduct::Codex,
        session_kind: SessionKind::Background,
        tmux_session_name: "tmux-1".to_string(),
        session_id: "session-1".to_string(),
        state: SessionState::Active,
        branch_name: None,
        last_summary: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    ctx.repo
        .upsert_session_without_commit(&ctx.project, &task.id, session.clone())
        .unwrap();
    ctx.write_sessions(vec![SessionRecord {
        project_id: ctx.project.id.clone(),
        task_id: task.id.clone(),
        task_title: task.title.clone(),
        session: session.clone(),
        notification_sent: false,
    }]);
    ctx.tmux_state
        .lock()
        .unwrap()
        .existing
        .insert("tmux-1".to_string());

    let reused = ctx
        .session_service
        .start_session(
            &ctx.project,
            &task,
            CodingAgentProduct::Codex,
            SessionKind::Background,
        )
        .unwrap();

    assert_eq!(reused.session_id, "session-1");
    assert!(ctx.tmux_state.lock().unwrap().created.is_empty());
}

#[test]
fn session_service_finalize_returns_none_for_unknown_session() {
    let ctx = ServiceContext::new();

    let result = ctx
        .session_service
        .finalize_attached_session("missing-session")
        .unwrap();

    assert!(result.is_none());
}

struct ServiceContext {
    project: ProjectRecord,
    repo: TaskStore,
    session_service: SessionService,
    tmux_state: Arc<Mutex<FakeTmuxState>>,
    sessions_path: PathBuf,
}

impl ServiceContext {
    fn new() -> Self {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::mem::forget(temp);
        let state_root = root.join(".youbot");
        let state_history = StateHistory::new(state_root.clone());
        let project_catalog = ProjectCatalog::new(state_root.clone());
        let repo = TaskStore::new(state_root.clone());
        let review = SessionReviewService::new(repo.clone());
        let tmux_state = Arc::new(Mutex::new(FakeTmuxState::default()));
        let session_service = SessionService::with_handles(
            state_root.clone(),
            120,
            Arc::new(FakeTmux {
                state: tmux_state.clone(),
            }),
            review,
            Arc::new(FakeNotifier::default()),
            repo.clone(),
            state_history,
            project_catalog.clone(),
        );
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "example".to_string(),
            path: root.join("repo"),
            created_at: Utc::now(),
            config: ProjectConfig::default(),
        };
        std::fs::create_dir_all(&project.path).unwrap();
        project_catalog
            .save(std::slice::from_ref(&project))
            .unwrap();

        Self {
            project,
            repo,
            session_service,
            tmux_state,
            sessions_path: state_root.join("sessions.json"),
        }
    }

    fn create_task(&self, description: &str) -> TaskRecord {
        self.repo
            .create_task(&self.project, "Task title", description)
            .unwrap()
    }

    fn write_sessions(&self, sessions: Vec<SessionRecord>) {
        if let Some(parent) = self.sessions_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        state_files::atomic_write(
            &self.sessions_path,
            serde_json::to_string_pretty(&sessions).unwrap(),
        )
        .unwrap();
    }
}

#[derive(Default)]
struct FakeTmuxState {
    existing: HashSet<String>,
    created: Vec<(String, PathBuf, String, bool)>,
    transcripts: HashMap<String, String>,
}

struct FakeTmux {
    state: Arc<Mutex<FakeTmuxState>>,
}

impl TerminalSessionOps for FakeTmux {
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

    fn send_keys(&self, _session_name: &str, _input: &str) -> Result<()> {
        Ok(())
    }

    fn enable_monitor_silence(&self, _session_name: &str, _seconds: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeNotifier;

impl NotificationSink for FakeNotifier {
    fn notify(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}
