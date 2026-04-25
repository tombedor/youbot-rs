pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod tui;
pub mod ui;

pub fn run() -> anyhow::Result<()> {
    let mut app = app::App::load()?;
    tui::run(&mut app)
}
