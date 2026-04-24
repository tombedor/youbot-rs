pub mod app;
pub mod coding_agent_supervisor;
pub mod config;
pub mod controllers;
pub mod models;
pub mod notifier;
pub mod project_registry;
pub mod session_manager;
pub mod task_repository;
pub mod tmux_client;
pub mod tui;
pub mod views;

pub fn run() -> anyhow::Result<()> {
    let mut app = app::App::load()?;
    tui::run(&mut app)
}
