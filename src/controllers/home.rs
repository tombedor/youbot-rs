use crate::app::App;
use crate::models::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Down => {
            if !app.projects.is_empty() {
                app.selected_project = (app.selected_project + 1) % app.projects.len();
                app.reload_tasks()?;
            }
        }
        KeyCode::Up => {
            if !app.projects.is_empty() {
                app.selected_project = if app.selected_project == 0 {
                    app.projects.len() - 1
                } else {
                    app.selected_project - 1
                };
                app.reload_tasks()?;
            }
        }
        KeyCode::Enter => {
            if app.selected_project().is_some() {
                app.route = Route::ProjectDetail;
            }
        }
        KeyCode::Char('b') => {
            if let Some(session_name) = app.attach_selected_project_background_session() {
                return Ok(Some(session_name));
            }
            app.status = "No active background session for selected project".to_string();
        }
        KeyCode::Char('a') => {
            app.reset_add_repo_form();
            app.route = Route::AddRepo;
        }
        KeyCode::Char('r') => {
            app.refresh()?;
            app.status = "Refreshed".to_string();
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
        AddRepoField, AddRepoForm, AppConfig, AgentSessionRef, ProjectConfig, ProjectRecord, Route,
        SessionKind, SessionRecord, SessionState,
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
    fn b_attaches_selected_project_background_session() {
        let mut app = test_app();
        app.sessions = vec![SessionRecord {
            project_id: app.projects[0].id.clone(),
            task_id: "task-1".to_string(),
            task_title: "Background task".to_string(),
            session: AgentSessionRef {
                product: crate::models::CodingAgentProduct::Codex,
                session_kind: SessionKind::Background,
                tmux_session_name: "tmux-task-1".to_string(),
                session_id: "session-1".to_string(),
                state: SessionState::Active,
                branch_name: None,
                last_summary: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        }];

        let session = handle(&mut app, key_char('b')).unwrap().unwrap();

        assert_eq!(session, "tmux-task-1");
        assert_eq!(app.route, Route::LiveSession);
    }

    #[test]
    fn a_resets_form_and_enters_add_repo_route() {
        let mut app = test_app();
        app.add_repo_form.repo_input = "stale".to_string();

        handle(&mut app, key_char('a')).unwrap();

        assert_eq!(app.route, Route::AddRepo);
        assert!(app.add_repo_form.repo_input.is_empty());
        assert_eq!(app.add_repo_form.active_field, AddRepoField::RepoInput);
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

        App {
            config: config.clone(),
            route: Route::Home,
            projects: vec![project],
            tasks: Vec::new(),
            selected_project: 0,
            selected_task: 0,
            add_repo_form: AddRepoForm {
                location_input: config.managed_repo_root.display().to_string(),
                programming_language: "rust".to_string(),
                active_field: AddRepoField::RepoInput,
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
