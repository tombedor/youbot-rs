use crate::app::App;
use crate::domain::{CodingAgentProduct, SessionKind};
use crate::ui::session_actions::{attach_selected_task_session, start_selected_task_session};
use crate::ui::state::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => app.set_route(Route::ProjectDetail),
        KeyCode::Char('l') => {
            let session_name = start_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Live,
                Route::TaskDetail,
            )?
            .expect("live session should always attach immediately");
            return Ok(Some(session_name));
        }
        KeyCode::Char('b') => {
            start_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Background,
                Route::TaskDetail,
            )?;
        }
        KeyCode::Char('a') => {
            if let Some(session_name) = attach_selected_task_session(
                app,
                CodingAgentProduct::Codex,
                SessionKind::Background,
            )? {
                return Ok(Some(session_name));
            }
        }
        KeyCode::Char('c') => {
            let session_name = start_selected_task_session(
                app,
                CodingAgentProduct::ClaudeCode,
                SessionKind::Live,
                Route::TaskDetail,
            )?
            .expect("live session should always attach immediately");
            return Ok(Some(session_name));
        }
        _ => {}
    }
    Ok(None)
}
