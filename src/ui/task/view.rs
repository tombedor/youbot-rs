use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);

    let header = app
        .selected_task()
        .map(|task| format!("{} [{}]", task.title, task.status.label()))
        .unwrap_or_else(|| "Task".to_string());
    frame.render_widget(
        Paragraph::new(header).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Task View")
                .title_bottom(
                    "l live codex  b background codex  a attach bg codex  c live claude  Esc back",
                ),
        ),
        chunks[0],
    );

    let description = app
        .selected_task()
        .map(|task| task.description.as_str())
        .unwrap_or("No task selected.");
    frame.render_widget(
        Paragraph::new(description)
            .block(Block::default().borders(Borders::ALL).title("Description")),
        chunks[1],
    );

    let items: Vec<ListItem<'_>> = app
        .selected_task()
        .map(|task| {
            if task.sessions.is_empty() {
                vec![ListItem::new("No sessions yet.")]
            } else {
                task.sessions
                    .iter()
                    .map(|session| {
                        let summary = session
                            .last_summary
                            .as_ref()
                            .map(|summary| summary.summary.as_str())
                            .unwrap_or("No summary");
                        ListItem::new(format!(
                            "{} {} [{}] {}",
                            session.product.label(),
                            session.session_kind.label(),
                            session.state.label(),
                            summary
                        ))
                    })
                    .collect()
            }
        })
        .unwrap_or_else(|| vec![ListItem::new("No task selected.")]);

    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("Sessions")),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(app.status.as_str()).block(Block::default().borders(Borders::TOP)),
        chunks[3],
    );
}
