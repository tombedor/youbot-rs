use crate::app::App;
use crate::models::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::path::PathBuf;

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => app.route = Route::Home,
        KeyCode::Tab => app.add_repo_form.create_new_repo = !app.add_repo_form.create_new_repo,
        KeyCode::Backspace => {
            app.add_repo_form.repo_path.pop();
        }
        KeyCode::Char('r') => app.add_repo_form.remote_mode = (app.add_repo_form.remote_mode + 1) % 3,
        KeyCode::Char('d') => app.add_repo_form.dont_ask_again = !app.add_repo_form.dont_ask_again,
        KeyCode::Char(ch) => app.add_repo_form.repo_path.push(ch),
        KeyCode::Enter => {
            if app.add_repo_form.repo_path.trim().is_empty() {
                app.status = "Enter a repo path or repo name first".to_string();
                return Ok(None);
            }

            if app.add_repo_form.create_new_repo {
                let root = app.config.managed_repo_root.clone();
                let name = app.add_repo_form.repo_path.trim().to_string();
                let language = if app.add_repo_form.programming_language.is_empty() {
                    "rust".to_string()
                } else {
                    app.add_repo_form.programming_language.clone()
                };
                app.project_registry
                    .create_new_repo(&root, &name, &language, false)?;
            } else {
                app.project_registry
                    .add_existing_repo(PathBuf::from(app.add_repo_form.repo_path.trim()), false)?;
            }
            app.projects = app.project_registry.load()?;
            app.selected_project = app.projects.len().saturating_sub(1);
            app.reload_tasks()?;
            app.add_repo_form = Default::default();
            app.route = Route::Home;
            app.status = "Project added".to_string();
        }
        _ => {}
    }
    Ok(None)
}
