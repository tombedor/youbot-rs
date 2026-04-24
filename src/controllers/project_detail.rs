use crate::app::App;
use crate::models::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
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
            let title = format!("Task {}", app.tasks.len() + 1);
            app.create_task(title)?;
        }
        KeyCode::Char('s') => {
            app.cycle_task_status()?;
        }
        _ => {}
    }
    Ok(None)
}
