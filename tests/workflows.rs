use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use youbot_rs::app::App;
use youbot_rs::coding_agent_supervisor::CodingAgentSupervisor;
use youbot_rs::models::{
    AddRepoForm, AddRepoStep, AppConfig, CodingAgentProduct, Route, SessionKind,
};
use youbot_rs::notifier::NotifySink;
use youbot_rs::project_registry::ProjectRegistry;
use youbot_rs::session_manager::SessionManager;
use youbot_rs::task_repository::TaskRepository;
use youbot_rs::tmux_client::TmuxOps;

#[test]
fn app_toggle_project_auto_merge_persists_to_registry() {
    let temp = tempdir().unwrap();
    let (mut app, _tmux_state) = test_app(temp.path());
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();
    let project = app
        .project_registry
        .add_existing_repo(&repo_path, false)
        .unwrap();
    app.projects = app.project_registry.load().unwrap();
    app.selected_project = 0;

    app.toggle_selected_project_auto_merge().unwrap();

    let reloaded = app.project_registry.load().unwrap();
    assert_eq!(project.id, reloaded[0].id);
    assert!(reloaded[0].config.auto_merge);
    assert_eq!(app.status, "Project set to auto-merge");
}

#[test]
fn create_task_start_session_and_reload_state_round_trips() {
    let temp = tempdir().unwrap();
    let (mut app, tmux_state) = test_app(temp.path());
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();
    app.project_registry
        .add_existing_repo(&repo_path, false)
        .unwrap();
    app.projects = app.project_registry.load().unwrap();
    app.selected_project = 0;

    app.create_task("Investigate the background worker drift")
        .unwrap();
    let session_name = app
        .start_session(CodingAgentProduct::Codex, SessionKind::Background)
        .unwrap();

    let tasks = app.task_repository.load_tasks(&app.projects[0]).unwrap();
    let sessions = app.session_manager.load_sessions().unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(
        tasks[0].description,
        "Investigate the background worker drift"
    );
    assert_eq!(tasks[0].sessions.len(), 1);
    assert_eq!(tasks[0].sessions[0].tmux_session_name, session_name);
    assert_eq!(sessions.len(), 1);

    let registry = ProjectRegistry::new(app.config.state_root.clone());
    let repo = TaskRepository::new(app.config.state_root.clone(), registry.clone());
    let supervisor = CodingAgentSupervisor::new(repo.clone());
    let manager = SessionManager::with_handles(
        app.config.state_root.clone(),
        120,
        Arc::new(FakeTmux { state: tmux_state }),
        supervisor,
        Arc::new(FakeNotifier::default()),
        repo,
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
    let project_registry = ProjectRegistry::new(state_root.clone());
    let task_repository = TaskRepository::new(state_root.clone(), project_registry.clone());
    let supervisor = CodingAgentSupervisor::new(task_repository.clone());
    let tmux_state = Arc::new(Mutex::new(FakeTmuxState::default()));
    let session_manager = SessionManager::with_handles(
        state_root.clone(),
        120,
        Arc::new(FakeTmux {
            state: tmux_state.clone(),
        }),
        supervisor.clone(),
        Arc::new(FakeNotifier::default()),
        task_repository.clone(),
        project_registry.clone(),
    );

    let app = App {
        config: config.clone(),
        route: Route::Home,
        projects: Vec::new(),
        tasks: Vec::new(),
        selected_project: 0,
        selected_task: 0,
        add_repo_form: AddRepoForm {
            step: AddRepoStep::ModeChoice,
            location_input: config.managed_repo_root.display().to_string(),
            ..AddRepoForm::default()
        },
        creating_task: false,
        task_draft: String::new(),
        status: "Ready".to_string(),
        should_quit: false,
        sessions: Vec::new(),
        supervisor,
        project_registry,
        task_repository,
        session_manager,
    };
    (app, tmux_state)
}

#[derive(Default)]
struct FakeTmuxState {
    existing: HashSet<String>,
    created: Vec<(String, PathBuf, String, bool)>,
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

impl NotifySink for FakeNotifier {
    fn notify(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}
