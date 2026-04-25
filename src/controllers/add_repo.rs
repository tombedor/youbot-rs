use crate::app::App;
use crate::config;
use crate::models::{AddRepoField, Route};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => {
            app.reset_add_repo_form();
            app.route = Route::Home;
        }
        KeyCode::Tab => {
            app.add_repo_form.active_field = match app.add_repo_form.active_field {
                AddRepoField::RepoInput => AddRepoField::LocationInput,
                AddRepoField::LocationInput => AddRepoField::RepoInput,
            };
        }
        KeyCode::Char('m') => {
            app.add_repo_form.create_new_repo = !app.add_repo_form.create_new_repo;
            app.add_repo_form.active_field = AddRepoField::RepoInput;
        }
        KeyCode::Char('r') if app.add_repo_form.create_new_repo => {
            app.add_repo_form.remote_mode = (app.add_repo_form.remote_mode + 1) % 3
        }
        KeyCode::Char('w') if app.add_repo_form.create_new_repo => {
            app.add_repo_form.create_location_policy =
                (app.add_repo_form.create_location_policy + 1) % 3
        }
        KeyCode::Char('l') if app.add_repo_form.create_new_repo => {
            app.add_repo_form.programming_language =
                next_language(&app.add_repo_form.programming_language).to_string();
        }
        KeyCode::Char('p') => {
            app.add_repo_form.auto_merge = !app.add_repo_form.auto_merge;
        }
        KeyCode::Backspace => {
            active_input(app).pop();
        }
        KeyCode::Char(ch) => active_input(app).push(ch),
        KeyCode::Enter => {
            let mut success_message = "Project added".to_string();
            if app.add_repo_form.create_new_repo {
                if let Some(message) = save_new_repo(app)? {
                    success_message = message;
                }
            } else {
                let path = app.add_repo_form.repo_input.trim();
                if path.is_empty() {
                    app.status = "Enter an existing repo path first".to_string();
                    return Ok(None);
                }
                app.project_registry
                    .add_existing_repo(PathBuf::from(path), app.add_repo_form.auto_merge)?;
            }
            app.projects = app.project_registry.load()?;
            app.selected_project = app.projects.len().saturating_sub(1);
            app.reload_tasks()?;
            app.reset_add_repo_form();
            app.route = Route::Home;
            app.status = success_message;
        }
        _ => {}
    }
    Ok(None)
}

fn next_language(current: &str) -> &'static str {
    match current.to_ascii_lowercase().as_str() {
        "" => "rust",
        "rust" => "python",
        "python" => "typescript",
        "typescript" => "none",
        _ => "rust",
    }
}

fn active_input(app: &mut App) -> &mut String {
    match app.add_repo_form.active_field {
        AddRepoField::RepoInput => &mut app.add_repo_form.repo_input,
        AddRepoField::LocationInput => &mut app.add_repo_form.location_input,
    }
}

fn save_new_repo(app: &mut App) -> Result<Option<String>> {
    let name = app.add_repo_form.repo_input.trim();
    if name.is_empty() {
        app.status = "Enter a repo name first".to_string();
        return Ok(None);
    }

    let root = app.add_repo_form.location_input.trim();
    if root.is_empty() {
        app.status = "Enter a create location first".to_string();
        return Ok(None);
    }

    let root = PathBuf::from(root);
    let language = if app.add_repo_form.programming_language.is_empty() {
        "rust".to_string()
    } else {
        app.add_repo_form.programming_language.clone()
    };
    app.project_registry.create_new_repo(
        &root,
        name,
        &language,
        app.add_repo_form.auto_merge,
        app.add_repo_form.remote_mode,
    )?;

    if matches!(app.add_repo_form.create_location_policy, 0 | 2) {
        app.config.managed_repo_root = root;
        config::save(&app.config)?;
        return Ok(Some(
            "Project added and default repo location updated".to_string(),
        ));
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
    fn create_new_repo_updates_default_location_when_policy_requires_it() {
        let temp = tempdir().unwrap();
        let create_root = temp.path().join("managed");
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;
        app.add_repo_form = AddRepoForm {
            repo_input: "demo".to_string(),
            location_input: create_root.display().to_string(),
            create_new_repo: true,
            programming_language: "rust".to_string(),
            create_location_policy: 0,
            remote_mode: 2,
            auto_merge: true,
            active_field: AddRepoField::RepoInput,
        };

        handle(&mut app, key_enter()).unwrap();

        assert_eq!(app.route, Route::Home);
        assert_eq!(app.projects.len(), 1);
        assert!(create_root.join("demo").exists());
        assert_eq!(app.config.managed_repo_root, create_root);
        assert!(app.projects[0].config.auto_merge);
    }

    #[test]
    fn attach_existing_repo_uses_selected_merge_mode() {
        let temp = tempdir().unwrap();
        let repo_path = temp.path().join("existing");
        std::fs::create_dir_all(&repo_path).unwrap();
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;
        app.add_repo_form.repo_input = repo_path.display().to_string();
        app.add_repo_form.auto_merge = true;

        handle(&mut app, key_enter()).unwrap();

        assert_eq!(app.projects.len(), 1);
        assert_eq!(app.projects[0].path, repo_path);
        assert!(app.projects[0].config.auto_merge);
    }

    #[test]
    fn tab_switches_active_add_repo_field() {
        let temp = tempdir().unwrap();
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;
        assert_eq!(app.add_repo_form.active_field, AddRepoField::RepoInput);

        handle(&mut app, key_tab()).unwrap();

        assert_eq!(app.add_repo_form.active_field, AddRepoField::LocationInput);
    }

    fn test_app(root: &Path) -> App {
        let state_root = root.join(".youbot");
        let config = AppConfig {
            state_root: state_root.clone(),
            managed_repo_root: root.join("default-managed"),
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
            route: Route::Home,
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

    fn key_enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    fn key_tab() -> KeyEvent {
        KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)
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
