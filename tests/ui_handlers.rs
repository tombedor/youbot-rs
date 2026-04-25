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
use youbot_rs::domain::{
    AgentSessionRef, AppConfig, CodingAgentProduct, ProjectConfig, ProjectRecord, SessionKind,
    SessionState, TaskRecord, TaskStatus,
};
use youbot_rs::infrastructure::notification::NotificationSink;
use youbot_rs::infrastructure::project_catalog::ProjectCatalog;
use youbot_rs::infrastructure::state_history::StateHistory;
use youbot_rs::infrastructure::task_store::TaskStore;
use youbot_rs::infrastructure::tmux::TerminalSessionOps;
use youbot_rs::ui;
use youbot_rs::ui::state::{AddRepoStep, AppState, Route};

#[test]
fn home_n_enters_add_repo_and_resets_form() {
    let temp = tempdir().unwrap();
    let (mut app, _) = test_app(temp.path(), Route::Home);
    app.state.add_repo_form.repo_input = "stale".to_string();

    ui::home::handler::handle(&mut app, key('n')).unwrap();

    assert_eq!(app.route(), Route::AddRepo);
    assert!(app.add_repo_form().repo_input.is_empty());
    assert_eq!(app.add_repo_form().step, AddRepoStep::ModeChoice);
}

#[test]
fn add_repo_mode_choice_accepts_numeric_selection() {
    let temp = tempdir().unwrap();
    let (mut app, _) = test_app(temp.path(), Route::AddRepo);

    ui::add_repo::handler::handle(&mut app, key('2')).unwrap();
    ui::add_repo::handler::handle(&mut app, enter()).unwrap();

    assert!(app.add_repo_form().create_new_repo);
    assert_eq!(app.add_repo_form().step, AddRepoStep::NewName);
}

#[test]
fn add_repo_existing_flow_registers_project() {
    let temp = tempdir().unwrap();
    let repo_path = temp.path().join("repo");
    std::fs::create_dir_all(&repo_path).unwrap();
    let (mut app, _) = test_app(temp.path(), Route::AddRepo);
    app.state.add_repo_form.step = AddRepoStep::ExistingPath;
    app.state.add_repo_form.location_input = repo_path.display().to_string();
    app.state.add_repo_form.auto_merge = true;

    ui::add_repo::handler::handle(&mut app, enter()).unwrap();
    ui::add_repo::handler::handle(&mut app, enter()).unwrap();

    assert_eq!(app.route(), Route::Home);
    assert_eq!(app.projects().len(), 1);
    assert!(app.projects()[0].config.auto_merge);
}

#[test]
fn project_detail_n_and_enter_creates_task() {
    let temp = tempdir().unwrap();
    let (mut app, _) = test_app(temp.path(), Route::ProjectDetail);

    ui::project_detail::handler::handle(&mut app, key('n')).unwrap();
    for ch in "Implement handler test".chars() {
        ui::project_detail::handler::handle(&mut app, key(ch)).unwrap();
    }
    ui::project_detail::handler::handle(&mut app, enter()).unwrap();

    assert!(!app.is_creating_task());
    assert_eq!(app.tasks().len(), 1);
    assert_eq!(app.tasks()[0].description, "Implement handler test");
}

#[test]
fn project_detail_status_selection_sets_explicit_status() {
    let temp = tempdir().unwrap();
    let (mut app, _) = test_app(temp.path(), Route::ProjectDetail);
    let project = app.selected_project().cloned().unwrap();
    app.services
        .task_service
        .create_task(&project, "desc")
        .unwrap();
    app.reload_tasks().unwrap();

    ui::project_detail::handler::handle(&mut app, key('s')).unwrap();
    ui::project_detail::handler::handle(&mut app, key('3')).unwrap();

    assert!(!app.is_choosing_status());
    assert_eq!(app.tasks()[0].status, TaskStatus::Complete);
    assert_eq!(app.status(), "Task status set to COMPLETE");
}

#[test]
fn task_esc_returns_to_project_detail() {
    let temp = tempdir().unwrap();
    let (mut app, _) = test_app(temp.path(), Route::TaskDetail);

    ui::task::handler::handle(&mut app, esc()).unwrap();

    assert_eq!(app.route(), Route::ProjectDetail);
}

fn test_app(root: &Path, route: Route) -> (App, Arc<Mutex<FakeTmuxState>>) {
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
        Arc::new(FakeNotifier),
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
    state.route = route;

    let project = ProjectRecord {
        id: "project-1".to_string(),
        name: "example".to_string(),
        path: root.join("project-1"),
        created_at: Utc::now(),
        config: ProjectConfig::default(),
    };
    std::fs::create_dir_all(&project.path).unwrap();
    state.projects = vec![project];

    if matches!(route, Route::TaskDetail) {
        state.tasks = vec![TaskRecord {
            id: "task-1".to_string(),
            title: "Task".to_string(),
            description: "desc".to_string(),
            status: TaskStatus::Todo,
            sessions: vec![AgentSessionRef {
                product: CodingAgentProduct::Codex,
                session_kind: SessionKind::Background,
                tmux_session_name: "tmux-task-1".to_string(),
                session_id: "session-1".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        }];
    }

    (App::from_parts(services, state), tmux_state)
}

fn key(ch: char) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char(ch),
        crossterm::event::KeyModifiers::NONE,
    )
}

fn enter() -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Enter,
        crossterm::event::KeyModifiers::NONE,
    )
}

fn esc() -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    )
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

struct FakeNotifier;

impl NotificationSink for FakeNotifier {
    fn notify(&self, _title: &str, _body: &str) -> Result<()> {
        Ok(())
    }
}
