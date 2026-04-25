use crate::app::App;
use crate::infrastructure::tmux::{TerminalSessionOps, TmuxTerminal};
use crate::ui;
use crate::ui::state::Route;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};
use std::time::{Duration, Instant};

pub fn run(app: &mut App) -> anyhow::Result<()> {
    let terminal = ratatui::init();
    let result = run_loop(terminal, app);
    ratatui::restore();
    result
}

fn run_loop(mut terminal: DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    let mut last_refresh = Instant::now();
    loop {
        maybe_refresh(app, &mut last_refresh, Instant::now());

        terminal.draw(|frame| render(frame, app))?;
        if app.should_quit {
            break;
        }

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }

        let input = event::read()?;
        let Some(session_name) = handle_input_event(app, input)? else {
            continue;
        };

        handle_attach_transition(app, &session_name, attach_with_tmux(app, &session_name));
        terminal = ratatui::init();
    }
    Ok(())
}

fn attach_with_tmux(app: &App, session_name: &str) -> Result<()> {
    ratatui::restore();
    let tmux = TmuxTerminal::new(app.config().tmux_socket_name.clone());
    tmux.attach(session_name)
}

fn maybe_refresh(app: &mut App, last_refresh: &mut Instant, now: Instant) {
    if now.duration_since(*last_refresh) >= Duration::from_secs(3) {
        let _ = app.refresh();
        *last_refresh = now;
    }
}

fn handle_input_event(app: &mut App, input: Event) -> Result<Option<String>> {
    let Event::Key(key) = input else {
        return Ok(None);
    };
    if key.kind != KeyEventKind::Press {
        return Ok(None);
    }
    app.handle_key(key)
}

fn handle_attach_transition(app: &mut App, session_name: &str, attach_result: Result<()>) {
    if let Err(error) = attach_result {
        app.status = format!("Failed to attach: {error:#}");
        return;
    }

    if let Ok(Some(status)) = app
        .services
        .session_service
        .finalize_attached_session(&app.projects, session_name)
    {
        app.status = status;
    } else {
        app.status = "Returned from live session".to_string();
    }
    let _ = app.refresh();
    app.route = Route::Home;
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    match app.route {
        Route::Home => ui::home::view::render(frame, app, area),
        Route::ProjectDetail => ui::project_detail::view::render(frame, app, area),
        Route::TaskDetail => ui::task::view::render(frame, app, area),
        Route::AddRepo => ui::add_repo::view::render(frame, app, area),
        Route::LiveSession => ui::live_session::view::render(frame, app, area),
    }
}
