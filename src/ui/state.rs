use crate::domain::{AppConfig, ProjectRecord, SessionRecord, TaskRecord};

#[derive(Debug, Clone)]
pub struct AddRepoForm {
    pub step: AddRepoStep,
    pub repo_input: String,
    pub location_input: String,
    pub create_new_repo: bool,
    pub programming_language: String,
    pub create_location_policy: usize,
    pub remote_mode: usize,
    pub auto_merge: bool,
}

impl Default for AddRepoForm {
    fn default() -> Self {
        Self {
            step: AddRepoStep::ModeChoice,
            repo_input: String::new(),
            location_input: String::new(),
            create_new_repo: false,
            programming_language: "rust".to_string(),
            create_location_policy: 0,
            remote_mode: 2,
            auto_merge: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddRepoStep {
    ModeChoice,
    ExistingPath,
    NewName,
    NewLocation,
    LocationPolicy,
    Language,
    Remote,
    MergeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    ProjectDetail,
    TaskDetail,
    AddRepo,
    LiveSession,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub route: Route,
    pub projects: Vec<ProjectRecord>,
    pub tasks: Vec<TaskRecord>,
    pub selected_project: usize,
    pub selected_task: usize,
    pub add_repo_form: AddRepoForm,
    pub creating_task: bool,
    pub task_draft: String,
    pub status: String,
    pub should_quit: bool,
    pub sessions: Vec<SessionRecord>,
}

impl AppState {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            route: Route::Home,
            projects: Vec::new(),
            tasks: Vec::new(),
            selected_project: 0,
            selected_task: 0,
            add_repo_form: AddRepoForm {
                step: AddRepoStep::ModeChoice,
                location_input: config.managed_repo_root.display().to_string(),
                programming_language: "rust".to_string(),
                ..AddRepoForm::default()
            },
            creating_task: false,
            task_draft: String::new(),
            status: "Ready".to_string(),
            should_quit: false,
            sessions: Vec::new(),
        }
    }
}
