use crate::app::{App, Focus};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::time::Duration;

pub fn run(app: &mut App) -> anyhow::Result<()> {
    let mut terminal = ratatui::init();

    let result = run_loop(&mut terminal, app);

    ratatui::restore();
    result
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    loop {
        app.poll_background();
        terminal.draw(|frame| render(frame, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(200))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match key.code {
                KeyCode::Char('q') if key.modifiers.is_empty() => app.should_quit = true,
                KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => app.open_palette(),
                KeyCode::Char('r') if key.modifiers.is_empty() => {
                    if let Err(error) = app.reload() {
                        app.status = format!("Reload failed: {error:#}");
                    }
                }
                KeyCode::Tab => app.cycle_focus(),
                KeyCode::Up => app.select_previous(),
                KeyCode::Down => app.select_next(),
                KeyCode::Enter => match app.focus {
                    Focus::Repos => app.activate_selected_repo(),
                    Focus::Commands => {
                        if let Err(error) = app.run_selected_command() {
                            app.status = format!("Command failed: {error:#}");
                        }
                    }
                    Focus::Palette => {
                        if let Err(error) = app.run_selected_palette_entry() {
                            app.status = format!("Palette action failed: {error:#}");
                        }
                    }
                    Focus::Input => {
                        if let Err(error) = app.submit_input() {
                            app.status = format!("Message save failed: {error:#}");
                        }
                    }
                },
                KeyCode::Esc => {
                    if app.focus == Focus::Palette {
                        app.close_palette();
                    } else {
                        app.clear_active_repo();
                    }
                }
                KeyCode::Backspace if app.focus == Focus::Input => {
                    app.input.pop();
                }
                KeyCode::Backspace if app.focus == Focus::Palette => {
                    app.palette_query.pop();
                }
                KeyCode::Char('u')
                    if app.focus == Focus::Input && key.modifiers == KeyModifiers::CONTROL =>
                {
                    app.input.clear();
                }
                KeyCode::Char('u')
                    if app.focus == Focus::Palette && key.modifiers == KeyModifiers::CONTROL =>
                {
                    app.palette_query.clear();
                }
                KeyCode::Char(ch) if app.focus == Focus::Input && key.modifiers.is_empty() => {
                    app.input.push(ch);
                }
                KeyCode::Char(ch) if app.focus == Focus::Palette && key.modifiers.is_empty() => {
                    app.palette_query.push(ch);
                    app.palette_cursor = 0;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn render(frame: &mut Frame<'_>, app: &App) {
    let has_active_repo = app.active_repo().is_some();
    let chunks = if has_active_repo {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30),
                Constraint::Min(50),
                Constraint::Length(42),
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(50)])
            .split(frame.area())
    };

    render_repos(frame, app, chunks[0]);
    render_chat(frame, app, chunks[1]);
    if has_active_repo {
        render_repo_panel(frame, app, chunks[2]);
    }
    if app.focus == Focus::Palette {
        render_palette(frame, app);
    }
}

fn render_repos(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem<'_>> = if app.repos.is_empty() {
        vec![ListItem::new(
            "No repos configured in ~/.youbot/config.json",
        )]
    } else {
        app.repos
            .iter()
            .enumerate()
            .map(|(index, repo)| {
                let is_active = app.active_repo == Some(index);
                let marker = if is_active { "*" } else { " " };
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{marker} ")),
                    Span::styled(
                        repo.name.clone(),
                        Style::default().add_modifier(if is_active {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                    ),
                    Span::raw(format!(" [{}]", repo.status.label())),
                ]))
            })
            .collect()
    };

    let list = List::new(items)
        .block(block(
            "Repos",
            app.focus == Focus::Repos,
            Some("Up/Down navigate  Enter activate  Ctrl-P palette"),
        ))
        .highlight_style(highlight_style())
        .highlight_symbol("› ");

    let mut state = ratatui::widgets::ListState::default();
    if !app.repos.is_empty() {
        state.select(Some(app.repo_cursor));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_chat(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(4),
            Constraint::Length(2),
        ])
        .split(area);

    let lines: Vec<Line<'_>> = if app.conversation.record().messages.is_empty() {
        vec![Line::from("Conversation is empty.")]
    } else {
        app.conversation
            .record()
            .messages
            .iter()
            .rev()
            .take(14)
            .rev()
            .flat_map(|message| {
                let header = Line::from(vec![
                    Span::styled(
                        format!("[{}] ", message.role.label()),
                        Style::default()
                            .fg(role_color(message.role))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(&message.created_at),
                ]);
                let body = Line::from(message.content.clone());
                [header, body, Line::from("")]
            })
            .collect()
    };

    let chat = Paragraph::new(Text::from(lines))
        .block(block(
            "Chat",
            false,
            Some(if app.is_processing() {
                "Processing request..."
            } else {
                "Conversation history and command output"
            }),
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(chat, rows[0]);

    let input = Paragraph::new(app.input.as_str())
        .block(block(
            "Input",
            app.focus == Focus::Input,
            Some("Type a message and press Enter  Ctrl-P palette"),
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(input, rows[1]);

    let status = Paragraph::new(app.status.as_str())
        .block(Block::default().borders(Borders::TOP))
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(status, rows[2]);
}

fn render_repo_panel(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let Some(repo) = app.active_repo() else {
        frame.render_widget(Clear, area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Length(8),
            Constraint::Length(8),
        ])
        .split(area);

    let summary = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            repo.name.clone(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("status: {}", repo.status.label())),
        Line::from(repo.path.display().to_string()),
        Line::from(
            app.active_overview
                .as_ref()
                .map(|overview| overview.subtitle.clone())
                .unwrap_or_else(|| "No overview loaded".to_string()),
        ),
    ]))
    .block(block("Active Repo", false, None))
    .wrap(Wrap { trim: false });
    frame.render_widget(summary, rows[0]);

    render_overview(frame, app, rows[1]);
    render_quick_actions(frame, app, rows[2]);
    render_activity(frame, app, rows[3]);
}

fn render_quick_actions(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let items: Vec<ListItem<'_>> = app
        .active_overview
        .as_ref()
        .map(|overview| {
            overview
                .quick_actions
                .iter()
                .map(|action| {
                    ListItem::new(format!("{}  just {}", action.title, action.command_name))
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| vec![ListItem::new("No adapter quick actions configured")]);

    let list = List::new(items)
        .block(block(
            "Quick Actions",
            app.focus == Focus::Commands,
            Some("Up/Down navigate  Enter run  Ctrl-P full palette"),
        ))
        .highlight_style(highlight_style())
        .highlight_symbol("› ");

    let mut state = ratatui::widgets::ListState::default();
    let actions_len = app
        .active_overview
        .as_ref()
        .map(|overview| overview.quick_actions.len())
        .unwrap_or(0);
    if actions_len > 0 {
        state.select(Some(app.command_cursor.min(actions_len - 1)));
    }
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_overview(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let Some(overview) = &app.active_overview else {
        let empty = Paragraph::new("No overview loaded.")
            .block(block("Overview", false, Some("Select or refresh repo")))
            .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    };

    let constraints = vec![Constraint::Length(6); overview.cards.len().max(1)];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    if overview.cards.is_empty() {
        let empty = Paragraph::new(overview.subtitle.as_str())
            .block(block("Overview", false, Some(&overview.subtitle)))
            .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    for (index, card) in overview.cards.iter().enumerate() {
        if index >= rows.len() {
            break;
        }
        let body: Vec<Line<'_>> = card
            .lines
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect();
        let paragraph = Paragraph::new(Text::from(body))
            .block(block(&card.title, false, None))
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, rows[index]);
    }
}

fn render_activity(frame: &mut Frame<'_>, app: &App, area: ratatui::layout::Rect) {
    let lines: Vec<Line<'_>> = if let Some(activity) = &app.latest_activity {
        let mut body = vec![
            Line::from(format!("backend: {}", activity.backend_name)),
            Line::from(format!("status: {}", activity.status)),
            Line::from(format!("target: {}", activity.target_repo_id)),
        ];
        body.extend(
            activity
                .recent_entries
                .iter()
                .take(3)
                .map(|entry| Line::from(entry.as_str())),
        );
        body
    } else {
        vec![Line::from("No coding-agent activity yet.")]
    };

    let widget = Paragraph::new(Text::from(lines))
        .block(block(
            "Agent Activity",
            false,
            Some("Live coding-agent run state"),
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_palette(frame: &mut Frame<'_>, app: &App) {
    let area = centered_rect(frame.area(), 70, 60);
    frame.render_widget(Clear, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(area);

    let input = Paragraph::new(app.palette_query.as_str())
        .block(block(
            "Command Palette",
            true,
            Some("Type to filter  Enter run  Esc close"),
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(input, rows[0]);

    let entries = app.filtered_palette_entries();
    let items = if entries.is_empty() {
        vec![ListItem::new("No actions match the current filter.")]
    } else {
        entries
            .iter()
            .map(|entry| {
                ListItem::new(vec![
                    Line::from(Span::styled(
                        entry.title.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!("{}  {}", entry.scope, entry.subtitle)),
                ])
            })
            .collect()
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(highlight_style())
        .highlight_symbol("› ");
    let mut state = ratatui::widgets::ListState::default();
    if !entries.is_empty() {
        state.select(Some(app.palette_cursor.min(entries.len() - 1)));
    }
    frame.render_stateful_widget(list, rows[1], &mut state);
}

fn centered_rect(
    area: ratatui::layout::Rect,
    width_pct: u16,
    height_pct: u16,
) -> ratatui::layout::Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height_pct) / 2),
            Constraint::Percentage(height_pct),
            Constraint::Percentage((100 - height_pct) / 2),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - width_pct) / 2),
            Constraint::Percentage(width_pct),
            Constraint::Percentage((100 - width_pct) / 2),
        ])
        .split(vertical[1]);
    horizontal[1]
}

fn block<'a>(title: &'a str, focused: bool, footer: Option<&'a str>) -> Block<'a> {
    let border_style = if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let mut block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    if let Some(footer) = footer {
        block = block.title_bottom(Line::from(Span::styled(
            footer,
            Style::default().fg(Color::DarkGray),
        )));
    }
    block
}

fn highlight_style() -> Style {
    Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

fn role_color(role: crate::models::MessageRole) -> Color {
    match role {
        crate::models::MessageRole::User => Color::Green,
        crate::models::MessageRole::Assistant => Color::Cyan,
        crate::models::MessageRole::System => Color::Yellow,
        crate::models::MessageRole::Tool => Color::Magenta,
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::app::{App, Focus};
    use crate::conversation_store::ConversationStore;
    use crate::models::{
        CodingAgentActivity, CommandRecord, ConversationMessage, ConversationRecord, MessageRole,
        OverviewCard, QuickActionView, RepoClassification, RepoOverview, RepoRecord, RepoStatus,
        StructuredOutputFormat,
    };
    use insta::assert_snapshot;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;

    #[test]
    fn renders_global_chat_view() {
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        let mut app = App::for_test(sample_repos(), sample_conversation(), None, None);
        app.focus = Focus::Input;
        app.input = "show me my top 5 tasks".to_string();

        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_snapshot!("global_chat_view", format!("{:?}", terminal.backend()));
    }

    #[test]
    fn renders_active_repo_workspace() {
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        let mut app = App::for_test(
            sample_repos(),
            sample_conversation(),
            Some(1),
            Some(sample_overview()),
        );
        app.command_cursor = 1;
        app.latest_activity = Some(CodingAgentActivity {
            run_id: "run-123".to_string(),
            target_repo_id: "life_admin".to_string(),
            target_kind: "repo".to_string(),
            backend_name: "codex".to_string(),
            request_summary: "Review task presentation".to_string(),
            session_id: Some("session-xyz".to_string()),
            status: "running".to_string(),
            recent_entries: vec![
                "stdout: Opened adapter metadata".to_string(),
                "stdout: Adjusted quick actions".to_string(),
                "stdout: Rendering updated workspace".to_string(),
            ],
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_snapshot!("active_repo_workspace", format!("{:?}", terminal.backend()));
    }

    #[test]
    fn renders_command_palette_overlay() {
        let mut terminal = Terminal::new(TestBackend::new(120, 32)).unwrap();
        let mut app = App::for_test(
            sample_repos(),
            sample_conversation(),
            Some(1),
            Some(sample_overview()),
        );
        app.open_palette();
        app.palette_query = "task".to_string();
        app.focus = Focus::Palette;

        terminal.draw(|frame| render(frame, &app)).unwrap();
        assert_snapshot!(
            "command_palette_overlay",
            format!("{:?}", terminal.backend())
        );
    }

    fn sample_repos() -> Vec<RepoRecord> {
        vec![
            sample_repo(
                "job_search",
                "job_search",
                vec!["pipeline-status", "next-actions"],
            ),
            sample_repo(
                "life_admin",
                "life_admin",
                vec!["task-list", "task-digest", "cal-today"],
            ),
            sample_repo(
                "trader-bot",
                "trader-bot",
                vec!["research-program", "research-findings"],
            ),
        ]
    }

    fn sample_repo(repo_id: &str, name: &str, commands: Vec<&str>) -> RepoRecord {
        RepoRecord {
            repo_id: repo_id.to_string(),
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{repo_id}")),
            classification: RepoClassification::Integrated,
            status: RepoStatus::Ready,
            purpose_summary: None,
            tags: Vec::new(),
            preferred_commands: Vec::new(),
            commands: commands
                .into_iter()
                .map(|command_name| CommandRecord {
                    repo_id: repo_id.to_string(),
                    command_name: command_name.to_string(),
                    display_name: command_name.replace('-', " "),
                    description: Some(format!("Run {command_name}")),
                    invocation: vec!["just".to_string(), command_name.to_string()],
                    supports_structured_output: command_name.contains("task"),
                    structured_output_format: if command_name.contains("task") {
                        StructuredOutputFormat::Json
                    } else {
                        StructuredOutputFormat::Unknown
                    },
                    tags: Vec::new(),
                })
                .collect(),
            last_scanned_at: None,
            last_active_at: None,
            adapter_id: Some(format!("{repo_id}-adapter")),
            preferred_backend: None,
        }
    }

    fn sample_conversation() -> ConversationStore {
        ConversationStore::from_record(ConversationRecord {
            conversation_id: "conversation-1".to_string(),
            updated_at: "2026-04-23T12:00:00Z".to_string(),
            last_response_id: None,
            messages: vec![
                ConversationMessage {
                    message_id: "msg-1".to_string(),
                    role: MessageRole::User,
                    content: "show me my top 5 tasks".to_string(),
                    created_at: "2026-04-23T11:59:00Z".to_string(),
                },
                ConversationMessage {
                    message_id: "msg-2".to_string(),
                    role: MessageRole::Assistant,
                    content: "Top tasks loaded from life_admin.".to_string(),
                    created_at: "2026-04-23T11:59:03Z".to_string(),
                },
            ],
        })
    }

    fn sample_overview() -> RepoOverview {
        RepoOverview {
            subtitle: "life_admin workspace".to_string(),
            cards: vec![
                OverviewCard {
                    title: "Counts".to_string(),
                    lines: vec![
                        "urgent: 2".to_string(),
                        "high: 4".to_string(),
                        "medium: 7".to_string(),
                        "low: 3".to_string(),
                    ],
                },
                OverviewCard {
                    title: "Top Tasks".to_string(),
                    lines: vec![
                        "Review healthcare options [open, high]".to_string(),
                        "Renew license [open, urgent]".to_string(),
                        "Send tax documents [blocked, high]".to_string(),
                    ],
                },
            ],
            quick_actions: vec![
                QuickActionView {
                    title: "task digest".to_string(),
                    command_name: "task-digest".to_string(),
                    arguments: vec!["json".to_string()],
                },
                QuickActionView {
                    title: "task list".to_string(),
                    command_name: "task-list".to_string(),
                    arguments: vec!["5".to_string(), "json".to_string()],
                },
                QuickActionView {
                    title: "today".to_string(),
                    command_name: "cal-today".to_string(),
                    arguments: Vec::new(),
                },
            ],
        }
    }
}
