use crate::app::App;
use crate::config;
use crate::ui::state::{AddRepoStep, Route};
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
        AddRepoStep::ExistingPath | AddRepoStep::NewLocation => {
            &mut app.add_repo_form.location_input
        }
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
        app.services
            .project_catalog
            .add_existing_repo(PathBuf::from(path), app.add_repo_form.auto_merge)?;
        "Project added".to_string()
    };

    app.projects = app.services.project_catalog.load()?;
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
    app.services.project_catalog.create_new_repo(
        &root,
        app.add_repo_form.repo_input.trim(),
        &app.add_repo_form.programming_language,
        app.add_repo_form.auto_merge,
        app.add_repo_form.remote_mode,
    )?;

    if matches!(app.add_repo_form.create_location_policy, 0 | 2) {
        app.services.config.managed_repo_root = root;
        config::save(&app.services.config)?;
        Ok("Project added and default repo location updated".to_string())
    } else {
        Ok("Project added".to_string())
    }
}
