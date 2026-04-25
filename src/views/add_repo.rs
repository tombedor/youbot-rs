use crate::app::App;
use crate::models::AddRepoField;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mode = if app.add_repo_form.create_new_repo {
        "create new repo"
    } else {
        "attach existing repo"
    };
    let field_label = if app.add_repo_form.create_new_repo {
        "Repo name"
    } else {
        "Existing repo path"
    };
    let repo_marker = if app.add_repo_form.active_field == AddRepoField::RepoInput {
        ">"
    } else {
        " "
    };
    let location_marker = if app.add_repo_form.active_field == AddRepoField::LocationInput {
        ">"
    } else {
        " "
    };
    let mut lines = vec![
        format!("Mode: {mode}"),
        format!(
            "{repo_marker} {field_label}: {}",
            app.add_repo_form.repo_input
        ),
    ];

    if app.add_repo_form.create_new_repo {
        let remote = match app.add_repo_form.remote_mode {
            0 => "public",
            1 => "private",
            _ => "none",
        };
        let location_policy = match app.add_repo_form.create_location_policy {
            0 => "always create new repos here",
            1 => "just create this one here",
            _ => "just create this one and do not ask again",
        };
        lines.push(format!(
            "Programming language: {}",
            &app.add_repo_form.programming_language
        ));
        lines.push(format!("Create location policy: {location_policy}"));
        lines.push(format!("Remote: {remote}"));
        lines.push(format!(
            "{location_marker} Create location: {}",
            app.add_repo_form.location_input
        ));
        lines.push(format!(
            "Merge mode: {}",
            if app.add_repo_form.auto_merge {
                "auto-merge"
            } else {
                "open-pr"
            }
        ));
        lines.push(String::new());
        lines.push(
            "Type into the selected field. m toggles mode, Tab switches fields, l cycles language, w cycles location policy, r cycles remote, p toggles merge mode, Enter saves, Esc cancels.".to_string(),
        );
    } else {
        lines.push(format!(
            "Merge mode: {}",
            if app.add_repo_form.auto_merge {
                "auto-merge"
            } else {
                "open-pr"
            }
        ));
        lines.push(String::new());
        lines.push(
            "Type an existing repo path. m toggles mode, p toggles merge mode, Enter saves, Esc cancels.".to_string(),
        );
    }

    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title("Add Repo")),
        area,
    );
}
