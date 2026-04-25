use crate::app::App;
use crate::models::{CodingAgentProduct, Route, SessionKind};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => app.route = Route::ProjectDetail,
        KeyCode::Char('l') => {
            let session_name = app.start_session(CodingAgentProduct::Codex, SessionKind::Live)?;
            return Ok(Some(session_name));
        }
        KeyCode::Char('b') => {
            app.start_session(CodingAgentProduct::Codex, SessionKind::Background)?;
            app.route = Route::ProjectDetail;
        }
        KeyCode::Char('a') => {
            return app.attach_existing_session(CodingAgentProduct::Codex, SessionKind::Background);
        }
        KeyCode::Char('c') => {
            let session_name =
                app.start_session(CodingAgentProduct::ClaudeCode, SessionKind::Live)?;
            return Ok(Some(session_name));
        }
        _ => {}
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::handle;
    use crate::app::App;
    use crate::coding_agent_supervisor::CodingAgentSupervisor;
    use crate::models::{
        AddRepoForm, AddRepoStep, AppConfig, AgentSessionRef, CodingAgentProduct, ProjectConfig,
        ProjectRecord, Route, SessionKind, SessionState, TaskRecord, TaskStatus,
    };
    use crate::notifier::NotifySink;
    use crate::project_registry::ProjectRegistry;
    use crate::session_manager::SessionManager;
    use crate::task_repository::TaskRepository;
    use crate::tmux_client::TmuxOps;
    use anyhow::Result;
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn a_attaches_existing_background_session() {
        let mut app = test_app();

        let session = handle(&mut app, key_char('a')).unwrap().unwrap();

        assert_eq!(session, "tmux-task-1");
        assert_eq!(app.route, Route::LiveSession);
    }

    #[test]
    fn esc_returns_to_project_detail() {
        let mut app = test_app();

        handle(&mut app, key_esc()).unwrap();

        assert_eq!(app.route, Route::ProjectDetail);
    }

    fn test_app() -> App {
        let temp = tempdir().unwrap();
        let state_root = temp.path().join(".youbot");
        let config = AppConfig {
            state_root: state_root.clone(),
            managed_repo_root: temp.path().join("managed"),
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
            path: temp.path().join("repo"),
            created_at: Utc::now(),
            config: ProjectConfig::default(),
        };
        std::fs::create_dir_all(&project.path).unwrap();
        let task = TaskRecord {
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
        };

        App {
            config: config.clone(),
            route: Route::TaskDetail,
            projects: vec![project],
            tasks: vec![task],
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

    fn key_char(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)
    }

    fn key_esc() -> KeyEvent {
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
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
