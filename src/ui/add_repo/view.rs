use crate::app::App;
use crate::ui::state::AddRepoStep;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let summary = build_summary(app);
    let (question, answer, help) = match app.add_repo_form().step {
        AddRepoStep::ModeChoice => (
            "Is this an existing repo or a new repo?",
            if app.add_repo_form().create_new_repo {
                "1 Existing repo\n2 New repo\nSelected: New repo"
            } else {
                "1 Existing repo\n2 New repo\nSelected: Existing repo"
            }
            .to_string(),
            "Press 1-2 or Left/Right, then Enter. Esc cancels.".to_string(),
        ),
        AddRepoStep::ExistingPath => (
            "What is the existing repo path?",
            app.add_repo_form().location_input.clone(),
            "Type the repo path. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::NewName => (
            "What is the new repo name?",
            app.add_repo_form().repo_input.clone(),
            "Type the repo name. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::NewLocation => (
            "Where should the new repo be created?",
            app.add_repo_form().location_input.clone(),
            "Type the create location. Enter confirms. Esc cancels.".to_string(),
        ),
        AddRepoStep::LocationPolicy => (
            "Should this location become the default for new repos?",
            format!(
                "1 Always create new repos here\n2 Just create this one here\n3 Just create this one and do not ask again\nSelected: {}",
                location_policy_label(app.add_repo_form().create_location_policy)
            ),
            "Press 1-3 or Left/Right, then Enter. Esc cancels.".to_string(),
        ),
        AddRepoStep::Language => (
            "What programming language should be initialized?",
            format!(
                "1 rust\n2 python\n3 typescript\n4 none\nSelected: {}",
                app.add_repo_form().programming_language
            ),
            "Press 1-4 or Left/Right, then Enter. Esc cancels.".to_string(),
        ),
        AddRepoStep::Remote => (
            "Should a remote GitHub repo be created?",
            format!(
                "1 public\n2 private\n3 none\nSelected: {}",
                remote_label(app.add_repo_form().remote_mode)
            ),
            "Press 1-3 or Left/Right, then Enter. Esc cancels.".to_string(),
        ),
        AddRepoStep::MergeMode => (
            "Should this project auto-merge or open PRs?",
            format!(
                "1 auto-merge\n2 open-pr\nSelected: {}",
                merge_mode_label(app.add_repo_form().auto_merge)
            ),
            "Press 1-2 or Left/Right, then Enter saves. Esc cancels.".to_string(),
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
    let mode = if app.add_repo_form().create_new_repo {
        "new repo"
    } else {
        "existing repo"
    };
    format!(
        "Mode: {mode}\nRepo name: {}\nRepo path/location: {}\nLocation policy: {}\nLanguage: {}\nRemote: {}\nMerge mode: {}",
        blank_if_empty(&app.add_repo_form().repo_input),
        blank_if_empty(&app.add_repo_form().location_input),
        location_policy_label(app.add_repo_form().create_location_policy),
        app.add_repo_form().programming_language,
        remote_label(app.add_repo_form().remote_mode),
        merge_mode_label(app.add_repo_form().auto_merge),
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
    if auto_merge { "auto-merge" } else { "open-pr" }
}
