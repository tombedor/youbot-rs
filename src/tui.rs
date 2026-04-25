use crate::app::App;
use crate::models::Route;
use crate::tmux_client::TmuxOps;
use crate::views;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

pub fn run(app: &mut App) -> anyhow::Result<()> {
    let terminal = ratatui::init();
    let result = run_loop(terminal, app);
    ratatui::restore();
    result
}

fn run_loop(mut terminal: DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    let mut last_refresh = Instant::now();
    loop {
        maybe_refresh(app, &mut last_refresh, Instant::now());

        terminal.draw(|frame| render(frame, app))?;
        if app.should_quit {
            break;
        }

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let input = event::read()?;
        let Some(session_name) = handle_input_event(app, input)? else {
            continue;
        };

        handle_attach_transition(app, &session_name, attach_with_tmux(app, &session_name));
        terminal = ratatui::init();
    }
    Ok(())
}

fn attach_with_tmux(app: &App, session_name: &str) -> Result<()> {
    ratatui::restore();
    let tmux = crate::tmux_client::TmuxClient::new(app.config.tmux_socket_name.clone());
    tmux.attach(session_name)
}

fn maybe_refresh(app: &mut App, last_refresh: &mut Instant, now: Instant) {
    if now.duration_since(*last_refresh) >= Duration::from_secs(3) {
        let _ = app.refresh();
        *last_refresh = now;
    }
}

fn handle_input_event(app: &mut App, input: Event) -> Result<Option<String>> {
    let Event::Key(key) = input else {
        return Ok(None);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(None);
    }
    app.handle_key(key)
}

fn handle_attach_transition(app: &mut App, session_name: &str, attach_result: Result<()>) {
    if let Err(error) = attach_result {
        app.status = format!("Failed to attach: {error:#}");
        return;
    }

    if let Ok(Some(status)) = app
        .session_manager
        .finalize_attached_session(&app.projects, session_name)
    {
        app.status = status;
    } else {
        app.status = "Returned from live session".to_string();
    }
    let _ = app.refresh();
    app.route = Route::Home;
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    match app.route {
        Route::Home => views::home::render(frame, app, area),
        Route::ProjectDetail => views::project_detail::render(frame, app, area),
        Route::TaskDetail => views::task::render(frame, app, area),
        Route::AddRepo => views::add_repo::render(frame, app, area),
        Route::LiveSession => views::live_session::render(frame, app, area),
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_attach_transition, handle_input_event, maybe_refresh};
    use crate::app::App;
    use crate::coding_agent_supervisor::CodingAgentSupervisor;
    use crate::models::{
        AddRepoForm, AddRepoStep, AgentSessionRef, AppConfig, CodingAgentProduct, ProjectConfig,
        ProjectRecord, Route, SessionKind, SessionRecord, SessionState, TaskRecord, TaskStatus,
    };
    use crate::notifier::NotifySink;
    use crate::project_registry::ProjectRegistry;
    use crate::session_manager::SessionManager;
    use crate::task_repository::TaskRepository;
    use crate::tmux_client::TmuxOps;
    use anyhow::{Result, anyhow};
    use chrono::Utc;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::tempdir;

    #[test]
    fn maybe_refresh_updates_timestamp_and_reloads_state() {
        let mut app = test_app();
        let project = app.projects[0].clone();
        app.task_repository
            .create_task(&project, "Task title", "from disk")
            .unwrap();
        let mut last = Instant::now() - Duration::from_secs(4);
        let before = last;
        let now = Instant::now();

        maybe_refresh(&mut app, &mut last, now);

        assert!(last > before);
        assert_eq!(app.tasks.len(), 1);
    }

    #[test]
    fn handle_input_event_ignores_non_press_keys() {
        let mut app = test_app();

        let result = handle_input_event(
            &mut app,
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Release,
                state: crossterm::event::KeyEventState::NONE,
            }),
        )
        .unwrap();

        assert!(result.is_none());
        assert!(!app.should_quit);
    }

    #[test]
    fn handle_input_event_returns_attach_target_from_controller() {
        let mut app = test_app();
        app.route = Route::TaskDetail;
        app.tasks[0].sessions = vec![AgentSessionRef {
            product: CodingAgentProduct::Codex,
            session_kind: SessionKind::Background,
            tmux_session_name: "tmux-task-1".to_string(),
            session_id: "session-1".to_string(),
            state: SessionState::Active,
            branch_name: None,
            last_summary: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }];

        let result = handle_input_event(
            &mut app,
            Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
        )
        .unwrap();

        assert_eq!(result.as_deref(), Some("tmux-task-1"));
        assert_eq!(app.route, Route::LiveSession);
    }

    #[test]
    fn handle_attach_transition_sets_error_status_on_failure() {
        let mut app = test_app();

        handle_attach_transition(&mut app, "tmux-task-1", Err(anyhow!("boom")));

        assert!(app.status.contains("Failed to attach"));
    }

    #[test]
    fn handle_attach_transition_finalizes_session_and_returns_home() {
        let mut app = test_app();
        let project = app.projects[0].clone();
        let task = app
            .task_repository
            .create_task(&project, "Task title", "live task")
            .unwrap();
        let session = AgentSessionRef {
            product: CodingAgentProduct::Codex,
            session_kind: SessionKind::Live,
            tmux_session_name: "tmux-task-1".to_string(),
            session_id: "session-1".to_string(),
            state: SessionState::Active,
            branch_name: None,
            last_summary: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        app.task_repository
            .upsert_session(&project, &task.id, session.clone())
            .unwrap();
        write_sessions(
            &app,
            vec![SessionRecord {
                project_id: project.id.clone(),
                task_id: task.id.clone(),
                task_title: task.title.clone(),
                session,
                notification_sent: false,
            }],
        );

        handle_attach_transition(&mut app, "tmux-task-1", Ok(()));

        assert_eq!(app.route, Route::Home);
        assert!(app.status.contains("exited before transcript could be captured"));
    }

    fn test_app() -> App {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::mem::forget(temp);
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
        let session_manager = SessionManager::with_handles(
            state_root.clone(),
            120,
            Arc::new(NoopTmux),
            supervisor.clone(),
            Arc::new(NoopNotifier),
            task_repository.clone(),
            project_registry.clone(),
        );
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "example".to_string(),
            path: root.join("repo"),
            created_at: Utc::now(),
            config: ProjectConfig::default(),
        };
        std::fs::create_dir_all(&project.path).unwrap();
        project_registry.save(std::slice::from_ref(&project)).unwrap();

        App {
            config: config.clone(),
            route: Route::Home,
            projects: vec![project],
            tasks: vec![TaskRecord {
                id: "task-1".to_string(),
                title: "Task".to_string(),
                description: "desc".to_string(),
                status: TaskStatus::Todo,
                sessions: Vec::new(),
            }],
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
        }
    }

    fn write_sessions(app: &App, sessions: Vec<SessionRecord>) {
        let path = app.config.state_root.join("sessions.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_string_pretty(&sessions).unwrap()).unwrap();
    }

    struct NoopTmux;

    impl TmuxOps for NoopTmux {
        fn session_exists(&self, _session_name: &str) -> bool {
            false
        }

        fn create_session(
            &self,
            _session_name: &str,
            _cwd: &Path,
            _command: &str,
            _detached: bool,
        ) -> Result<()> {
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

    struct NoopNotifier;

    impl NotifySink for NoopNotifier {
        fn notify(&self, _title: &str, _body: &str) -> Result<()> {
            Ok(())
        }
    }
}
