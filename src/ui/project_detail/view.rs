use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(4),
            Constraint::Length(2),
        ])
        .split(area);

    let title = app
        .selected_project()
        .map(|project| {
            let merge_mode = if project.config.auto_merge {
                "auto-merge"
            } else {
                "open-pr"
            };
            format!("Project: {} ({merge_mode})", project.name)
        })
        .unwrap_or_else(|| "Project".to_string());
    frame.render_widget(
        Paragraph::new(title).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Project Detail")
                .title_bottom(
                    "n new task  s set status  l live  b bg  a attach bg  c claude  Enter task  m merge  Esc home",
                ),
        ),
        chunks[0],
    );

    let items: Vec<ListItem<'_>> = if app.tasks().is_empty() {
        vec![ListItem::new("No tasks. Press 'n' to create one.")]
    } else {
        app.tasks()
            .iter()
            .enumerate()
            .map(|(index, task)| {
                let marker = if index == app.selected_task_index() {
                    ">"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{marker} {} [{}]\n  {}",
                    task.title,
                    task.status.label(),
                    task.description
                ))
            })
            .collect()
    };
    frame.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("TODO")),
        chunks[1],
    );

    let task_input = if app.is_creating_task() {
        format!(
            "New task description: {}\nEnter saves, Esc cancels",
            app.task_draft()
        )
    } else if app.is_choosing_status() {
        "Choose task status:\n1 TODO\n2 IN PROGRESS\n3 COMPLETE\n4 WONT DO\nEsc cancels".to_string()
    } else {
        "Press 'n' to create a task, 's' to set status, or l/b/a/c for sessions.".to_string()
    };
    frame.render_widget(
        Paragraph::new(task_input)
            .block(Block::default().borders(Borders::ALL).title("Task Input")),
        chunks[2],
    );

    frame.render_widget(
        Paragraph::new(app.status()).block(Block::default().borders(Borders::TOP)),
        chunks[3],
    );
}
