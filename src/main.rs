mod adapter;
mod app;
mod coding_agent;
mod config;
mod conversation_store;
mod executor;
mod justfile_parser;
mod models;
mod openai_chat;
mod overview;
mod persistence;
mod registry;
mod router;
mod tui;

use anyhow::Result;

fn main() -> Result<()> {
    let mut app = app::App::load()?;
    tui::run(&mut app)
}
