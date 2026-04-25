use crate::app::App;
use crate::models::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    if key.code == KeyCode::Esc {
        app.route = Route::Home;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::handle;
    use crate::app::App;
    use crate::coding_agent_supervisor::CodingAgentSupervisor;
    use crate::models::{AddRepoField, AddRepoForm, AppConfig, Route};
    use crate::notifier::NotifySink;
    use crate::project_registry::ProjectRegistry;
    use crate::session_manager::SessionManager;
    use crate::task_repository::TaskRepository;
    use crate::tmux_client::TmuxOps;
    use anyhow::Result;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn esc_returns_to_home() {
        let mut app = test_app();

        handle(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)).unwrap();

        assert_eq!(app.route, Route::Home);
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

        App {
            config: config.clone(),
            route: Route::LiveSession,
            projects: Vec::new(),
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
