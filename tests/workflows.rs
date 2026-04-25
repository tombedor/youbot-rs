use anyhow::Result;
use chrono::Utc;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use youbot_rs::app::App;
use youbot_rs::application::context::AppServices;
use youbot_rs::application::project_service::ProjectService;
use youbot_rs::application::session_review_service::SessionReviewService;
use youbot_rs::application::session_service::SessionService;
use youbot_rs::application::task_service::TaskService;
use youbot_rs::domain::{AppConfig, CodingAgentProduct, ProjectConfig, ProjectRecord, SessionKind};
use youbot_rs::infrastructure::notification::NotificationSink;
use youbot_rs::infrastructure::project_catalog::ProjectCatalog;
use youbot_rs::infrastructure::state_history::StateHistory;
use youbot_rs::infrastructure::task_store::TaskStore;
use youbot_rs::infrastructure::tmux::TerminalSessionOps;
use youbot_rs::ui::state::AppState;

#[test]
fn app_toggle_project_auto_merge_persists_to_registry() {
    let temp = tempdir().unwrap();
    let (mut app, _tmux_state) = test_app(temp.path());
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();
    let added_project = app
        .services
        .project_service
        .add_existing_repo(&repo_path, false)
        .unwrap();
    app.state.projects = app.services.project_service.load_projects().unwrap();
    app.state.selected_project = 0;

    let project = app.selected_project().cloned().unwrap();
    app.services
        .project_service
        .set_auto_merge(&project.id, true)
        .unwrap();
    app.state.projects = app.services.project_service.load_projects().unwrap();
    app.set_status("Project set to auto-merge");

    let reloaded = app.services.project_service.load_projects().unwrap();
    assert_eq!(added_project.id, reloaded[0].id);
    assert!(reloaded[0].config.auto_merge);
    assert_eq!(app.status(), "Project set to auto-merge");
}

#[test]
fn create_task_start_session_and_reload_state_round_trips() {
    let temp = tempdir().unwrap();
    let (mut app, tmux_state) = test_app(temp.path());
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();
    app.services
        .project_service
        .add_existing_repo(&repo_path, false)
        .unwrap();
    app.state.projects = app.services.project_service.load_projects().unwrap();
    app.state.selected_project = 0;

    let project = app.selected_project().cloned().unwrap();
    let task = app
        .services
        .task_service
        .create_task(&project, "Investigate the background worker drift")
        .unwrap();
    app.reload_tasks().unwrap();
    let session = app
        .services
        .session_service
        .start_session(
            &project,
            &task,
            CodingAgentProduct::Codex,
            SessionKind::Background,
        )
        .unwrap();
    app.state.sessions = app.services.session_service.load_sessions().unwrap();
    app.reload_tasks().unwrap();
    let session_name = session.tmux_session_name.clone();

    let tasks = app
        .services
        .task_store
        .load_tasks(&app.projects()[0])
        .unwrap();
    let sessions = app.services.session_service.load_sessions().unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].description,
        "Investigate the background worker drift"
    );
    assert_eq!(tasks[0].sessions.len(), 1);
    assert_eq!(tasks[0].sessions[0].tmux_session_name, session_name);
    assert_eq!(sessions.len(), 1);

    let registry = ProjectCatalog::new(app.config().state_root.clone());
    let repo = TaskStore::new(app.config().state_root.clone());
    let supervisor = SessionReviewService::new(repo.clone());
    let state_history = StateHistory::new(app.config().state_root.clone());
    let manager = SessionService::with_handles(
        app.config().state_root.clone(),
        120,
        Arc::new(FakeTmux { state: tmux_state }),
        supervisor,
        Arc::new(FakeNotifier::default()),
        repo,
        state_history,
        registry,
    );
    let reloaded_sessions = manager.load_sessions().unwrap();

    assert_eq!(reloaded_sessions.len(), 1);
    assert_eq!(reloaded_sessions[0].session.tmux_session_name, session_name);
}

fn test_app(root: &Path) -> (App, Arc<Mutex<FakeTmuxState>>) {
    let state_root = root.join(".youbot");
    let config = AppConfig {
        state_root: state_root.clone(),
        managed_repo_root: root.join("managed"),
        tmux_socket_name: "youbot-test".to_string(),
        monitor_silence_seconds: 120,
    };
    let state_history = StateHistory::new(state_root.clone());
    let project_catalog = ProjectCatalog::new(state_root.clone());
    let task_store = TaskStore::new(state_root.clone());
    let project_service = ProjectService::new(project_catalog.clone(), state_history.clone());
    let task_service = TaskService::new(task_store.clone(), state_history.clone());
    let session_review_service = SessionReviewService::new(task_store.clone());
    let tmux_state = Arc::new(Mutex::new(FakeTmuxState::default()));
    let session_service = SessionService::with_handles(
        state_root.clone(),
        120,
        Arc::new(FakeTmux {
            state: tmux_state.clone(),
        }),
        session_review_service.clone(),
        Arc::new(FakeNotifier::default()),
        task_store.clone(),
        state_history.clone(),
        project_catalog.clone(),
    );

    let services = AppServices {
        config: config.clone(),
        state_history,
        project_catalog,
        task_store,
        project_service,
        task_service,
        session_review_service,
        session_service,
    };
    let mut state = AppState::new(&config);
    let project = ProjectRecord {
        id: "project-1".to_string(),
        name: "example".to_string(),
        path: root.join("project-1"),
        created_at: Utc::now(),
        config: ProjectConfig::default(),
    };
    std::fs::create_dir_all(&project.path).unwrap();
    state.projects = vec![project];

    (App::from_parts(services, state), tmux_state)
}

#[derive(Default)]
struct FakeTmuxState {
    existing: HashSet<String>,
    created: Vec<(String, PathBuf, String, bool)>,
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

    fn capture_pane(&self, _session_name: &str) -> Result<String> {
        Ok(String::new())
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
