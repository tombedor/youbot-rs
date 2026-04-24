use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let body = format!(
        "Attached session state is managed through tmux.\n\n{}\n\nPress Esc to return after leaving the tmux session.",
        app.status
    );
    frame.render_widget(
        Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title("Live Coding Session")),
        area,
    );
}
