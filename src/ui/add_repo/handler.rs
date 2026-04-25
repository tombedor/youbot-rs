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
            app.set_route(Route::Home);
        }
        KeyCode::Left | KeyCode::Right => handle_choice_cycle(app),
        KeyCode::Backspace => {
            active_input(app).pop();
        }
        KeyCode::Char(ch) => {
            if !handle_choice_char(app, ch) {
                handle_char(app, ch);
            }
        }
        KeyCode::Enter => submit_step(app)?,
        _ => {}
    }
    Ok(None)
}

fn handle_choice_cycle(app: &mut App) {
    match app.add_repo_form().step {
        AddRepoStep::ModeChoice => {
            app.add_repo_form_mut().create_new_repo = !app.add_repo_form().create_new_repo;
        }
        AddRepoStep::LocationPolicy => {
            let next = (app.add_repo_form().create_location_policy + 1) % 3;
            app.add_repo_form_mut().create_location_policy = next;
        }
        AddRepoStep::Language => {
            let next = next_language(&app.add_repo_form().programming_language).to_string();
            app.add_repo_form_mut().programming_language = next;
        }
        AddRepoStep::Remote => {
            let next = (app.add_repo_form().remote_mode + 1) % 3;
            app.add_repo_form_mut().remote_mode = next;
        }
        AddRepoStep::MergeMode => {
            app.add_repo_form_mut().auto_merge = !app.add_repo_form().auto_merge;
        }
        _ => {}
    }
}

fn handle_choice_char(app: &mut App, ch: char) -> bool {
    match app.add_repo_form().step {
        AddRepoStep::ModeChoice => match ch {
            '1' => {
                app.add_repo_form_mut().create_new_repo = false;
                true
            }
            '2' => {
                app.add_repo_form_mut().create_new_repo = true;
                true
            }
            _ => false,
        },
        AddRepoStep::LocationPolicy => match ch {
            '1' => {
                app.add_repo_form_mut().create_location_policy = 0;
                true
            }
            '2' => {
                app.add_repo_form_mut().create_location_policy = 1;
                true
            }
            '3' => {
                app.add_repo_form_mut().create_location_policy = 2;
                true
            }
            _ => false,
        },
        AddRepoStep::Language => match ch {
            '1' => {
                app.add_repo_form_mut().programming_language = "rust".to_string();
                true
            }
            '2' => {
                app.add_repo_form_mut().programming_language = "python".to_string();
                true
            }
            '3' => {
                app.add_repo_form_mut().programming_language = "typescript".to_string();
                true
            }
            '4' => {
                app.add_repo_form_mut().programming_language = "none".to_string();
                true
            }
            _ => false,
        },
        AddRepoStep::Remote => match ch {
            '1' => {
                app.add_repo_form_mut().remote_mode = 0;
                true
            }
            '2' => {
                app.add_repo_form_mut().remote_mode = 1;
                true
            }
            '3' => {
                app.add_repo_form_mut().remote_mode = 2;
                true
            }
            _ => false,
        },
        AddRepoStep::MergeMode => match ch {
            '1' => {
                app.add_repo_form_mut().auto_merge = true;
                true
            }
            '2' => {
                app.add_repo_form_mut().auto_merge = false;
                true
            }
            _ => false,
        },
        AddRepoStep::ExistingPath | AddRepoStep::NewName | AddRepoStep::NewLocation => false,
    }
}

fn handle_char(app: &mut App, ch: char) {
    match app.add_repo_form().step {
        AddRepoStep::ModeChoice
        | AddRepoStep::LocationPolicy
        | AddRepoStep::Language
        | AddRepoStep::Remote
        | AddRepoStep::MergeMode => {}
        _ => active_input(app).push(ch),
    }
}

fn active_input(app: &mut App) -> &mut String {
    match app.add_repo_form().step {
        AddRepoStep::ExistingPath | AddRepoStep::NewLocation => app.add_repo_location_input_mut(),
        AddRepoStep::NewName => app.add_repo_repo_input_mut(),
        AddRepoStep::ModeChoice
        | AddRepoStep::LocationPolicy
        | AddRepoStep::Language
        | AddRepoStep::Remote
        | AddRepoStep::MergeMode => app.add_repo_repo_input_mut(),
    }
}

fn submit_step(app: &mut App) -> Result<()> {
    match app.add_repo_form().step {
        AddRepoStep::ModeChoice => {
            app.add_repo_form_mut().step = if app.add_repo_form().create_new_repo {
                AddRepoStep::NewName
            } else {
                AddRepoStep::ExistingPath
            };
        }
        AddRepoStep::ExistingPath => {
            if app.add_repo_form().location_input.trim().is_empty() {
                app.set_status("Enter an existing repo path first");
            } else {
                app.add_repo_form_mut().step = AddRepoStep::MergeMode;
            }
        }
        AddRepoStep::NewName => {
            if app.add_repo_form().repo_input.trim().is_empty() {
                app.set_status("Enter a repo name first");
            } else {
                app.add_repo_form_mut().step = AddRepoStep::NewLocation;
            }
        }
        AddRepoStep::NewLocation => {
            if app.add_repo_form().location_input.trim().is_empty() {
                app.set_status("Enter a create location first");
            } else {
                app.add_repo_form_mut().step = AddRepoStep::LocationPolicy;
            }
        }
        AddRepoStep::LocationPolicy => app.add_repo_form_mut().step = AddRepoStep::Language,
        AddRepoStep::Language => app.add_repo_form_mut().step = AddRepoStep::Remote,
        AddRepoStep::Remote => app.add_repo_form_mut().step = AddRepoStep::MergeMode,
        AddRepoStep::MergeMode => save_form(app)?,
    }
    Ok(())
}

fn save_form(app: &mut App) -> Result<()> {
    let success_message = if app.add_repo_form().create_new_repo {
        save_new_repo(app)?
    } else {
        let path = app.add_repo_form().location_input.trim();
        app.services
            .project_service
            .add_existing_repo(PathBuf::from(path), app.add_repo_form().auto_merge)?;
        "Project added".to_string()
    };

    app.reload_projects()?;
    app.select_last_project();
    app.reload_tasks()?;
    app.reset_add_repo_form();
    app.set_route(Route::Home);
    app.set_status(success_message);
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
    let root = PathBuf::from(app.add_repo_form().location_input.trim());
    app.services.project_service.create_new_repo(
        &root,
        app.add_repo_form().repo_input.trim(),
        &app.add_repo_form().programming_language,
        app.add_repo_form().auto_merge,
        app.add_repo_form().remote_mode,
    )?;

    if matches!(app.add_repo_form().create_location_policy, 0 | 2) {
        app.services.config.managed_repo_root = root;
        config::save(&app.services.config)?;
        Ok("Project added and default repo location updated".to_string())
    } else {
        Ok("Project added".to_string())
    }
}
