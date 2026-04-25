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
            Constraint::Length(8),
            Constraint::Length(2),
        ])
        .split(area);

    let items: Vec<ListItem<'_>> = if app.projects().is_empty() {
        vec![ListItem::new("No projects. Press 'n' to add one.")]
    } else {
        app.projects()
            .iter()
            .enumerate()
            .map(|(index, project)| {
                let marker = if index == app.selected_project_index() {
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
            .title_bottom("Enter project  a attach bg  n add repo  r refresh  q quit"),
    );
    frame.render_widget(list, chunks[0]);

    let latest = app
        .latest_session_for_selected_project()
        .map(|record| {
            format!(
                "Last session: {} :: {} {} [{}]",
                record.task_title,
                record.session.product.label(),
                record.session.session_kind.label(),
                record.session.state.label()
            )
        })
        .unwrap_or_else(|| "Last session: none".to_string());
    let background_rows: Vec<String> = app
        .selected_project()
        .map(|project| {
            app.sessions()
                .iter()
                .filter(|record| {
                    record.project_id == project.id
                        && record.session.session_kind.label() == "background"
                        && record.session.state.label() != "exited"
                })
                .map(|record| {
                    format!(
                        "{} :: {} [{}]",
                        record.task_title,
                        record.session.product.label(),
                        record.session.state.label()
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let body = if background_rows.is_empty() {
        format!("{latest}\n\nBackground sessions: none")
    } else {
        format!(
            "{latest}\n\nBackground sessions:\n{}",
            background_rows.join("\n")
        )
    };
    let sessions = Paragraph::new(body).block(
        Block::default()
            .title("Selected Project Activity")
            .borders(Borders::ALL),
    );
    frame.render_widget(sessions, chunks[1]);

    let status = Paragraph::new(app.status())
        .block(Block::default().borders(Borders::TOP))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(status, chunks[2]);
}
