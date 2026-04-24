use crate::app::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mode = if app.add_repo_form.create_new_repo {
        "create new repo"
    } else {
        "attach existing repo"
    };
    let remote = match app.add_repo_form.remote_mode {
        0 => "public",
        1 => "private",
        _ => "none",
    };
    let body = format!(
        "Mode: {mode}\nPath or name: {}\nLanguage: {}\nRemote: {remote}\nDon't ask again: {}\n\nType a path or repo name. Tab toggles mode, r cycles remote, d toggles don't-ask-again, Enter saves, Esc cancels.",
        app.add_repo_form.repo_path,
        if app.add_repo_form.programming_language.is_empty() {
            "rust"
        } else {
            &app.add_repo_form.programming_language
        },
        app.add_repo_form.dont_ask_again
    );
    frame.render_widget(
        Paragraph::new(body)
            .block(Block::default().borders(Borders::ALL).title("Add Repo")),
        area,
    );
}
