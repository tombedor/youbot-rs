use crate::app::App;
use crate::models::{CodingAgentProduct, Route, SessionKind};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    match key.code {
        KeyCode::Esc => app.route = Route::ProjectDetail,
        KeyCode::Char('l') => {
            let session_name = app.start_session(CodingAgentProduct::Codex, SessionKind::Live)?;
            return Ok(Some(session_name));
        }
        KeyCode::Char('b') => {
            app.start_session(CodingAgentProduct::Codex, SessionKind::Background)?;
            app.route = Route::ProjectDetail;
        }
        KeyCode::Char('c') => {
            let session_name =
                app.start_session(CodingAgentProduct::ClaudeCode, SessionKind::Live)?;
            return Ok(Some(session_name));
        }
        _ => {}
    }
    Ok(None)
}
