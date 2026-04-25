use crate::app::App;
use crate::config;
use crate::models::{AddRepoStep, Route};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => {
            app.reset_add_repo_form();
            app.route = Route::Home;
        }
        KeyCode::Left | KeyCode::Right => handle_choice_cycle(app),
        KeyCode::Backspace => {
            active_input(app).pop();
        }
        KeyCode::Char(ch) => handle_char(app, ch),
        KeyCode::Enter => submit_step(app)?,
        _ => {}
    }
    Ok(None)
}

fn handle_choice_cycle(app: &mut App) {
    match app.add_repo_form.step {
        AddRepoStep::ModeChoice => {
            app.add_repo_form.create_new_repo = !app.add_repo_form.create_new_repo;
        }
        AddRepoStep::LocationPolicy => {
            app.add_repo_form.create_location_policy =
                (app.add_repo_form.create_location_policy + 1) % 3;
        }
        AddRepoStep::Language => {
            app.add_repo_form.programming_language =
                next_language(&app.add_repo_form.programming_language).to_string();
        }
        AddRepoStep::Remote => {
            app.add_repo_form.remote_mode = (app.add_repo_form.remote_mode + 1) % 3;
        }
        AddRepoStep::MergeMode => {
            app.add_repo_form.auto_merge = !app.add_repo_form.auto_merge;
        }
        _ => {}
    }
}

fn handle_char(app: &mut App, ch: char) {
    match app.add_repo_form.step {
        AddRepoStep::ModeChoice
        | AddRepoStep::LocationPolicy
        | AddRepoStep::Language
        | AddRepoStep::Remote
        | AddRepoStep::MergeMode => {}
        _ => active_input(app).push(ch),
    }
}

fn active_input(app: &mut App) -> &mut String {
    match app.add_repo_form.step {
        AddRepoStep::ExistingPath | AddRepoStep::NewLocation => &mut app.add_repo_form.location_input,
        AddRepoStep::NewName => &mut app.add_repo_form.repo_input,
        AddRepoStep::ModeChoice
        | AddRepoStep::LocationPolicy
        | AddRepoStep::Language
        | AddRepoStep::Remote
        | AddRepoStep::MergeMode => &mut app.add_repo_form.repo_input,
    }
}

fn submit_step(app: &mut App) -> Result<()> {
    match app.add_repo_form.step {
        AddRepoStep::ModeChoice => {
            app.add_repo_form.step = if app.add_repo_form.create_new_repo {
                AddRepoStep::NewName
            } else {
                AddRepoStep::ExistingPath
            };
        }
        AddRepoStep::ExistingPath => {
            if app.add_repo_form.location_input.trim().is_empty() {
                app.status = "Enter an existing repo path first".to_string();
            } else {
                app.add_repo_form.step = AddRepoStep::MergeMode;
            }
        }
        AddRepoStep::NewName => {
            if app.add_repo_form.repo_input.trim().is_empty() {
                app.status = "Enter a repo name first".to_string();
            } else {
                app.add_repo_form.step = AddRepoStep::NewLocation;
            }
        }
        AddRepoStep::NewLocation => {
            if app.add_repo_form.location_input.trim().is_empty() {
                app.status = "Enter a create location first".to_string();
            } else {
                app.add_repo_form.step = AddRepoStep::LocationPolicy;
            }
        }
        AddRepoStep::LocationPolicy => app.add_repo_form.step = AddRepoStep::Language,
        AddRepoStep::Language => app.add_repo_form.step = AddRepoStep::Remote,
        AddRepoStep::Remote => app.add_repo_form.step = AddRepoStep::MergeMode,
        AddRepoStep::MergeMode => save_form(app)?,
    }
    Ok(())
}

fn save_form(app: &mut App) -> Result<()> {
    let success_message = if app.add_repo_form.create_new_repo {
        save_new_repo(app)?
    } else {
        let path = app.add_repo_form.location_input.trim();
        app.project_registry
            .add_existing_repo(PathBuf::from(path), app.add_repo_form.auto_merge)?;
        "Project added".to_string()
    };

    app.projects = app.project_registry.load()?;
    app.selected_project = app.projects.len().saturating_sub(1);
    app.reload_tasks()?;
    app.reset_add_repo_form();
    app.route = Route::Home;
    app.status = success_message;
    Ok(())
}

fn next_language(current: &str) -> &'static str {
    match current.to_ascii_lowercase().as_str() {
        "rust" => "python",
        "python" => "typescript",
        "typescript" => "none",
        _ => "rust",
    }
}

fn save_new_repo(app: &mut App) -> Result<String> {
    let root = PathBuf::from(app.add_repo_form.location_input.trim());
    app.project_registry.create_new_repo(
        &root,
        app.add_repo_form.repo_input.trim(),
        &app.add_repo_form.programming_language,
        app.add_repo_form.auto_merge,
        app.add_repo_form.remote_mode,
    )?;

    if matches!(app.add_repo_form.create_location_policy, 0 | 2) {
        app.config.managed_repo_root = root;
        config::save(&app.config)?;
        Ok("Project added and default repo location updated".to_string())
    } else {
        Ok("Project added".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::handle;
    use crate::app::App;
    use crate::coding_agent_supervisor::CodingAgentSupervisor;
    use crate::models::{AddRepoForm, AddRepoStep, AppConfig, Route};
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
    fn mode_choice_advances_sequentially_to_existing_path() {
        let temp = tempdir().unwrap();
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;

        handle(&mut app, key_enter()).unwrap();

        assert_eq!(app.add_repo_form.step, AddRepoStep::ExistingPath);
    }

    #[test]
    fn mode_choice_can_switch_to_new_repo_without_typing_conflict() {
        let temp = tempdir().unwrap();
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;

        handle(&mut app, key_right()).unwrap();
        handle(&mut app, key_enter()).unwrap();

        assert!(app.add_repo_form.create_new_repo);
        assert_eq!(app.add_repo_form.step, AddRepoStep::NewName);
    }

    #[test]
    fn create_new_repo_updates_default_location_when_policy_requires_it() {
        let temp = tempdir().unwrap();
        let create_root = temp.path().join("managed");
        let mut app = test_app(temp.path());
        app.route = Route::AddRepo;
        app.add_repo_form.step = AddRepoStep::MergeMode;
        app.add_repo_form.create_new_repo = true;
        app.add_repo_form.repo_input = "demo".to_string();
        app.add_repo_form.location_input = create_root.display().to_string();
        app.add_repo_form.programming_language = "rust".to_string();
        app.add_repo_form.create_location_policy = 0;
        app.add_repo_form.remote_mode = 2;
        app.add_repo_form.auto_merge = true;

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
        app.add_repo_form.step = AddRepoStep::MergeMode;
        app.add_repo_form.location_input = repo_path.display().to_string();
        app.add_repo_form.auto_merge = true;

        handle(&mut app, key_enter()).unwrap();

        assert_eq!(app.projects.len(), 1);
        assert_eq!(app.projects[0].path, std::fs::canonicalize(&repo_path).unwrap());
        assert!(app.projects[0].config.auto_merge);
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

    fn key_right() -> KeyEvent {
        KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
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
