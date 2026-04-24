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
        KeyCode::Char('a') => app.route = Route::AddRepo,
        KeyCode::Char('r') => {
            app.refresh()?;
            app.status = "Refreshed".to_string();
        }
        _ => {}
    }
    Ok(None)
}
