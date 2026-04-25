use crate::app::App;
use crate::models::AddRepoStep;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let summary = build_summary(app);
    let (question, answer, help) = match app.add_repo_form.step {
        AddRepoStep::ModeChoice => (
            "Is this an existing repo or a new repo?",
            if app.add_repo_form.create_new_repo {
                "New repo"
            } else {
                "Existing repo"
            }
            .to_string(),
            "Left/Right changes the choice. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::ExistingPath => (
            "What is the existing repo path?",
            app.add_repo_form.location_input.clone(),
            "Type the repo path. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::NewName => (
            "What is the new repo name?",
            app.add_repo_form.repo_input.clone(),
            "Type the repo name. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::NewLocation => (
            "Where should the new repo be created?",
            app.add_repo_form.location_input.clone(),
            "Type the create location. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::LocationPolicy => (
            "Should this location become the default for new repos?",
            location_policy_label(app.add_repo_form.create_location_policy).to_string(),
            "Left/Right changes the choice. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::Language => (
            "What programming language should be initialized?",
            app.add_repo_form.programming_language.clone(),
            "Left/Right changes the choice. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::Remote => (
            "Should a remote GitHub repo be created?",
            remote_label(app.add_repo_form.remote_mode).to_string(),
            "Left/Right changes the choice. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::MergeMode => (
            "Should this project auto-merge or open PRs?",
            merge_mode_label(app.add_repo_form.auto_merge).to_string(),
            "Left/Right changes the choice. Enter saves. Esc cancels.".to_string(),
        ),
    };

    let body = format!("{summary}\n\nQuestion:\n{question}\n\nAnswer:\n{answer}\n\n{help}");
    frame.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Add Repo Wizard"),
        ),
        area,
    );
}

fn build_summary(app: &App) -> String {
    let mode = if app.add_repo_form.create_new_repo {
        "new repo"
    } else {
        "existing repo"
    };
    format!(
        "Mode: {mode}\nRepo name: {}\nRepo path/location: {}\nLocation policy: {}\nLanguage: {}\nRemote: {}\nMerge mode: {}",
        blank_if_empty(&app.add_repo_form.repo_input),
        blank_if_empty(&app.add_repo_form.location_input),
        location_policy_label(app.add_repo_form.create_location_policy),
        app.add_repo_form.programming_language,
        remote_label(app.add_repo_form.remote_mode),
        merge_mode_label(app.add_repo_form.auto_merge),
    )
}

fn blank_if_empty(value: &str) -> &str {
    if value.is_empty() { "-" } else { value }
}

fn location_policy_label(value: usize) -> &'static str {
    match value {
        0 => "always create new repos here",
        1 => "just create this one here",
        _ => "just create this one and do not ask again",
    }
}

fn remote_label(value: usize) -> &'static str {
    match value {
        0 => "public",
        1 => "private",
        _ => "none",
    }
}

fn merge_mode_label(auto_merge: bool) -> &'static str {
    if auto_merge {
        "auto-merge"
    } else {
        "open-pr"
    }
}
