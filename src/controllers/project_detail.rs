use crate::app::App;
use crate::models::{CodingAgentProduct, Route, SessionKind};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    if app.creating_task {
        match key.code {
            KeyCode::Esc => app.cancel_task_creation(),
            KeyCode::Backspace => {
                app.task_draft.pop();
            }
            KeyCode::Enter => {
                let description = app.task_draft.trim().to_string();
                if description.is_empty() {
                    app.status = "Task description cannot be empty".to_string();
                } else {
                    app.create_task(description)?;
                }
            }
            KeyCode::Char(ch) => app.task_draft.push(ch),
            _ => {}
        }
        return Ok(None);
    }

    match key.code {
        KeyCode::Esc => app.route = Route::Home,
        KeyCode::Down => {
            if !app.tasks.is_empty() {
                app.selected_task = (app.selected_task + 1) % app.tasks.len();
            }
        }
        KeyCode::Up => {
            if !app.tasks.is_empty() {
                app.selected_task = if app.selected_task == 0 {
                    app.tasks.len() - 1
                } else {
                    app.selected_task - 1
                };
            }
        }
        KeyCode::Enter => {
            if app.selected_task().is_some() {
                app.route = Route::TaskDetail;
            }
        }
        KeyCode::Char('n') => {
            app.begin_task_creation();
        }
        KeyCode::Char('s') => {
            app.cycle_task_status()?;
        }
        KeyCode::Char('a') => {
            return app.attach_existing_session(CodingAgentProduct::Codex, SessionKind::Background);
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
    use crate::models::{AppConfig, ProjectConfig, ProjectRecord, Route};
    use crate::notifier::Notifier;
    use crate::project_registry::ProjectRegistry;
    use crate::session_manager::SessionManager;
    use crate::task_repository::TaskRepository;
    use crate::tmux_client::TmuxClient;
    use chrono::Utc;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::tempdir;

    #[test]
    fn n_enters_task_creation_mode() {
        let mut app = test_app();
        handle(&mut app, key_char('n')).unwrap();
        assert!(app.creating_task);
        assert_eq!(app.status, "Enter a task description and press Enter");
    }

    #[test]
    fn enter_submits_typed_task_title() {
        let mut app = test_app();
        handle(&mut app, key_char('n')).unwrap();
        for ch in "Implement task input".chars() {
            handle(&mut app, key_char(ch)).unwrap();
        }
        handle(&mut app, key_enter()).unwrap();

        assert!(!app.creating_task);
        assert_eq!(app.tasks.len(), 1);
        assert_eq!(app.tasks[0].description, "Implement task input");
        assert_eq!(app.tasks[0].title, "Implement task input");
    }

    fn test_app() -> App {
        let temp = tempdir().unwrap();
        let state_root = temp.path().join(".youbot");
        let config = AppConfig {
            state_root: state_root.clone(),
            managed_repo_root: state_root.join("managed_repos"),
            tmux_socket_name: "youbot-test".to_string(),
            monitor_silence_seconds: 120,
        };
        let project_registry = ProjectRegistry::new(state_root.clone());
        let task_repository = TaskRepository::new(state_root.clone(), project_registry.clone());
        let supervisor = CodingAgentSupervisor::new(task_repository.clone());
        let session_manager = SessionManager::new(
            state_root.clone(),
            120,
            TmuxClient::new("youbot-test"),
            supervisor.clone(),
            Notifier,
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
            config,
            route: Route::ProjectDetail,
            projects: vec![project],
            tasks: Vec::new(),
            selected_project: 0,
            selected_task: 0,
            add_repo_form: Default::default(),
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

    fn key_enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }
}
