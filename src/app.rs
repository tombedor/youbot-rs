use crate::application::context::AppServices;
use crate::domain::{AppConfig, ProjectRecord, SessionRecord, TaskRecord, TaskStatus};
use crate::ui;
use crate::ui::state::{AddRepoForm, AppState, ProjectDetailState, Route};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

pub struct App {
    pub services: AppServices,
    pub state: AppState,
}

impl App {
    pub fn from_parts(services: AppServices, state: AppState) -> Self {
        Self { services, state }
    }

    pub fn load() -> Result<Self> {
        let services = AppServices::load()?;
        let mut state = AppState::new(&services.config);
        state.projects = services.project_service.load_projects()?;
        state.tasks = state
            .projects
            .first()
            .map(|project| services.task_service.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        state.sessions = services.session_service.load_sessions().unwrap_or_default();
        Ok(Self { services, state })
    }

    pub fn config(&self) -> &AppConfig {
        &self.services.config
    }

    pub fn route(&self) -> Route {
        self.state.route
    }

    pub fn set_route(&mut self, route: Route) {
        self.state.route = route;
    }

    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    pub fn request_quit(&mut self) {
        self.state.should_quit = true;
    }

    pub fn status(&self) -> &str {
        &self.state.status
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.state.status = status.into();
    }

    pub fn projects(&self) -> &[ProjectRecord] {
        &self.state.projects
    }

    pub fn tasks(&self) -> &[TaskRecord] {
        &self.state.tasks
    }

    pub fn sessions(&self) -> &[SessionRecord] {
        &self.state.sessions
    }

    pub fn add_repo_form(&self) -> &AddRepoForm {
        &self.state.add_repo_form
    }

    pub fn add_repo_form_mut(&mut self) -> &mut AddRepoForm {
        &mut self.state.add_repo_form
    }

    pub fn add_repo_repo_input_mut(&mut self) -> &mut String {
        &mut self.state.add_repo_form.repo_input
    }

    pub fn add_repo_location_input_mut(&mut self) -> &mut String {
        &mut self.state.add_repo_form.location_input
    }

    pub fn project_detail_state(&self) -> &ProjectDetailState {
        &self.state.project_detail_state
    }

    pub fn project_detail_state_mut(&mut self) -> &mut ProjectDetailState {
        &mut self.state.project_detail_state
    }

    pub fn is_creating_task(&self) -> bool {
        matches!(
            self.state.project_detail_state,
            ProjectDetailState::CreatingTask { .. }
        )
    }

    pub fn is_choosing_status(&self) -> bool {
        matches!(
            self.state.project_detail_state,
            ProjectDetailState::ChoosingStatus
        )
    }

    pub fn task_draft(&self) -> &str {
        match &self.state.project_detail_state {
            ProjectDetailState::Browsing => "",
            ProjectDetailState::CreatingTask { draft } => draft,
            ProjectDetailState::ChoosingStatus => "",
        }
    }

    pub fn task_draft_mut(&mut self) -> Option<&mut String> {
        match &mut self.state.project_detail_state {
            ProjectDetailState::Browsing => None,
            ProjectDetailState::CreatingTask { draft } => Some(draft),
            ProjectDetailState::ChoosingStatus => None,
        }
    }

    pub fn begin_task_creation(&mut self) {
        self.state.project_detail_state = ProjectDetailState::CreatingTask {
            draft: String::new(),
        };
        self.set_status("Enter a task description and press Enter");
    }

    pub fn begin_status_selection(&mut self) {
        self.state.project_detail_state = ProjectDetailState::ChoosingStatus;
        self.set_status("Choose a status with 1-4, or Esc to cancel");
    }

    pub fn cancel_task_creation(&mut self) {
        self.state.project_detail_state = ProjectDetailState::Browsing;
        self.set_status("Task creation cancelled");
    }

    pub fn cancel_status_selection(&mut self) {
        self.state.project_detail_state = ProjectDetailState::Browsing;
        self.set_status("Status change cancelled");
    }

    pub fn complete_task_creation(&mut self, title: impl Into<String>) {
        self.state.project_detail_state = ProjectDetailState::Browsing;
        self.set_status(format!("Task created: {}", title.into()));
    }

    pub fn complete_status_selection(&mut self, status: TaskStatus) {
        self.state.project_detail_state = ProjectDetailState::Browsing;
        self.set_status(format!("Task status set to {}", status.label()));
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.reload_projects()?;
        self.reload_sessions()?;
        self.reload_tasks()
    }

    pub fn selected_project_index(&self) -> usize {
        self.state.selected_project
    }

    pub fn selected_task_index(&self) -> usize {
        self.state.selected_task
    }

    pub fn set_selected_project_index(&mut self, index: usize) {
        self.state.selected_project = index;
    }

    pub fn set_selected_task_index(&mut self, index: usize) {
        self.state.selected_task = index;
    }

    pub fn selected_project(&self) -> Option<&ProjectRecord> {
        self.state.projects.get(self.state.selected_project)
    }

    pub fn selected_task(&self) -> Option<&TaskRecord> {
        self.state.tasks.get(self.state.selected_task)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<Option<String>> {
        if key.code == KeyCode::Char('q') {
            self.request_quit();
            return Ok(None);
        }

        match self.state.route {
            Route::Home => ui::home::handler::handle(self, key),
            Route::ProjectDetail => ui::project_detail::handler::handle(self, key),
            Route::TaskDetail => ui::task::handler::handle(self, key),
            Route::AddRepo => ui::add_repo::handler::handle(self, key),
            Route::LiveSession => ui::live_session::handler::handle(self, key),
        }
    }

    pub fn latest_session_for_selected_project(&self) -> Option<&SessionRecord> {
        let project = self.selected_project()?;
        self.state
            .sessions
            .iter()
            .filter(|record| record.project_id == project.id)
            .max_by_key(|record| record.session.updated_at)
    }

    pub fn reload_tasks(&mut self) -> Result<()> {
        self.state.tasks = self
            .selected_project()
            .map(|project| self.services.task_service.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        if self.state.selected_task >= self.state.tasks.len() && !self.state.tasks.is_empty() {
            self.state.selected_task = self.state.tasks.len() - 1;
        }
        Ok(())
    }

    pub fn reload_projects(&mut self) -> Result<()> {
        self.state.projects = self.services.project_service.load_projects()?;
        if self.state.selected_project >= self.state.projects.len()
            && !self.state.projects.is_empty()
        {
            self.state.selected_project = self.state.projects.len() - 1;
        }
        Ok(())
    }

    pub fn reload_sessions(&mut self) -> Result<()> {
        self.state.sessions = self
            .services
            .session_service
            .load_sessions()
            .unwrap_or_default();
        Ok(())
    }

    pub fn poll_sessions(&mut self) {
        self.state.sessions = self.services.session_service.poll().unwrap_or_default();
    }

    pub fn replace_projects(&mut self, projects: Vec<ProjectRecord>) {
        self.state.projects = projects;
    }

    pub fn replace_sessions(&mut self, sessions: Vec<SessionRecord>) {
        self.state.sessions = sessions;
    }

    pub fn select_last_project(&mut self) {
        self.state.selected_project = self.state.projects.len().saturating_sub(1);
    }

    pub fn select_first_project(&mut self) {
        self.state.selected_project = 0;
    }

    pub fn reset_add_repo_form(&mut self) {
        self.state.add_repo_form = AppState::new(&self.services.config).add_repo_form;
    }
}
