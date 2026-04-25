use crate::app::App;
use crate::domain::SessionKind;
use crate::ui::state::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Down => {
            if !app.projects().is_empty() {
                app.set_selected_project_index(
                    (app.selected_project_index() + 1) % app.projects().len(),
                );
                app.reload_tasks()?;
            }
        }
        KeyCode::Up => {
            if !app.projects().is_empty() {
                app.set_selected_project_index(if app.selected_project_index() == 0 {
                    app.projects().len() - 1
                } else {
                    app.selected_project_index() - 1
                });
                app.reload_tasks()?;
            }
        }
        KeyCode::Enter => {
            if app.selected_project().is_some() {
                app.set_route(Route::ProjectDetail);
            }
        }
        KeyCode::Char('a') => {
            if let Some(session_name) = attach_selected_project_background_session(app) {
                return Ok(Some(session_name));
            }
            app.set_status("No active background session for selected project");
        }
        KeyCode::Char('n') => {
            app.reset_add_repo_form();
            app.set_route(Route::AddRepo);
        }
        KeyCode::Char('r') => {
            app.refresh()?;
            app.set_status("Refreshed");
        }
        _ => {}
    }
    Ok(None)
}

fn attach_selected_project_background_session(app: &mut App) -> Option<String> {
    let project = app.selected_project()?;
    let session_name = app
        .sessions()
        .iter()
        .filter(|record| {
            record.project_id == project.id
                && record.session.session_kind == SessionKind::Background
                && !matches!(record.session.state, crate::domain::SessionState::Exited)
        })
        .max_by_key(|record| record.session.updated_at)
        .map(|record| {
            (
                record.session.session_id.clone(),
                record.session.tmux_session_name.clone(),
            )
        })?;
    app.set_route(Route::LiveSession);
    app.set_status(format!("Session {}", session_name.0));
    Some(session_name.1)
}
