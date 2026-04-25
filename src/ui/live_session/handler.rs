use crate::app::App;
use crate::ui::state::Route;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle(app: &mut App, key: KeyEvent) -> Result<Option<String>> {
    if key.code == KeyCode::Esc {
        app.set_route(Route::Home);
    }
    Ok(None)
}
