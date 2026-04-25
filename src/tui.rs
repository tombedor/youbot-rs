use crate::app::App;
use crate::models::Route;
use crate::tmux_client::TmuxOps;
use crate::views;
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
        if last_refresh.elapsed() >= Duration::from_secs(3) {
            let _ = app.refresh();
            last_refresh = Instant::now();
        }

        terminal.draw(|frame| render(frame, app))?;
        if app.should_quit {
            break;
        }

        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        let attach_target = app.handle_key(key)?;
        if let Some(session_name) = attach_target {
            ratatui::restore();
            let tmux = crate::tmux_client::TmuxClient::new(app.config.tmux_socket_name.clone());
            let attach_result = tmux.attach(&session_name);
            terminal = ratatui::init();
            if let Err(error) = attach_result {
                app.status = format!("Failed to attach: {error:#}");
            } else {
                if let Ok(Some(status)) = app
                    .session_manager
                    .finalize_attached_session(&app.projects, &session_name)
                {
                    app.status = status;
                } else {
                    app.status = "Returned from live session".to_string();
                }
                let _ = app.refresh();
                app.route = Route::Home;
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    match app.route {
        Route::Home => views::home::render(frame, app, area),
        Route::ProjectDetail => views::project_detail::render(frame, app, area),
        Route::TaskDetail => views::task::render(frame, app, area),
        Route::AddRepo => views::add_repo::render(frame, app, area),
        Route::LiveSession => views::live_session::render(frame, app, area),
    }
}
