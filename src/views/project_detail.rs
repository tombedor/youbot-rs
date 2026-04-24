use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8), Constraint::Length(2)])
        .split(area);

    let title = app
        .selected_project()
        .map(|project| format!("Project: {}", project.name))
        .unwrap_or_else(|| "Project".to_string());
    frame.render_widget(
        Paragraph::new(title).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Project Detail")
                .title_bottom("n new task  s change status  Enter task  Esc home"),
        ),
        chunks[0],
    );

    let items: Vec<ListItem<'_>> = if app.tasks.is_empty() {
        vec![ListItem::new("No tasks. Press 'n' to create one.")]
    } else {
        app.tasks
            .iter()
            .enumerate()
            .map(|(index, task)| {
                let marker = if index == app.selected_task { ">" } else { " " };
                ListItem::new(format!("{marker} {} [{}]", task.title, task.status.label()))
            })
            .collect()
    };
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("TODO")),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(app.status.as_str()).block(Block::default().borders(Borders::TOP)),
        chunks[2],
    );
}
