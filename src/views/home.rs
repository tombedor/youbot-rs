use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(6),
            Constraint::Length(2),
        ])
        .split(area);

    let items: Vec<ListItem<'_>> = if app.projects.is_empty() {
        vec![ListItem::new("No projects. Press 'a' to add one.")]
    } else {
        app.projects
            .iter()
            .enumerate()
            .map(|(index, project)| {
                let marker = if index == app.selected_project {
                    ">"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{marker} {} ({})",
                    project.name,
                    project.path.display()
                ))
            })
            .collect()
    };
    let list = List::new(items).block(
        Block::default()
            .title("Home")
            .borders(Borders::ALL)
            .title_bottom("Enter project  a add repo  r refresh  q quit"),
    );
    frame.render_widget(list, chunks[0]);

    let session_rows: Vec<ListItem<'_>> = if app.sessions.is_empty() {
        vec![ListItem::new("No active background sessions.")]
    } else {
        app.sessions
            .iter()
            .filter(|record| record.session.session_kind.label() == "background")
            .map(|record| {
                ListItem::new(format!(
                    "{} :: {} [{}]",
                    record.task_title,
                    record.session.product.label(),
                    record.session.state.label()
                ))
            })
            .collect()
    };
    let sessions = List::new(session_rows).block(
        Block::default()
            .title("Background Sessions")
            .borders(Borders::ALL),
    );
    frame.render_widget(sessions, chunks[1]);

    let status = Paragraph::new(app.status.as_str())
        .block(Block::default().borders(Borders::TOP))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(status, chunks[2]);
}
