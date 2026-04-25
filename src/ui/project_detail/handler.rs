use crate::app::App;
use crate::domain::{CodingAgentProduct, SessionKind};
use crate::ui::state::Route;
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
        KeyCode::Char('m') => {
            app.toggle_selected_project_auto_merge()?;
        }
        KeyCode::Char('a') => {
            return app.attach_existing_session(CodingAgentProduct::Codex, SessionKind::Background);
        }
        _ => {}
    }
    Ok(None)
}
