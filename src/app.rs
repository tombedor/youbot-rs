use crate::application::context::AppServices;
use crate::domain::{
    AppConfig, CodingAgentProduct, ProjectRecord, SessionKind, SessionRecord, SessionState,
    TaskRecord, TaskStatus,
};
use crate::ui;
use crate::ui::state::{AppState, Route};
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent};
use std::ops::{Deref, DerefMut};

pub struct App {
    pub services: AppServices,
    pub state: AppState,
}

impl Deref for App {
    type Target = AppState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for App {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}

impl App {
    pub fn from_parts(services: AppServices, state: AppState) -> Self {
        Self { services, state }
    }

    pub fn load() -> Result<Self> {
        let services = AppServices::load()?;
        let mut state = AppState::new(&services.config);
        state.projects = services.project_catalog.load()?;
        state.tasks = state
            .projects
            .first()
            .map(|project| services.task_store.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        state.sessions = services.session_service.load_sessions().unwrap_or_default();
        Ok(Self { services, state })
    }

    pub fn config(&self) -> &AppConfig {
        &self.services.config
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.projects = self.services.project_catalog.load()?;
        self.sessions = self
            .services
            .session_service
            .poll(&self.projects)
            .unwrap_or_default();
        self.reload_tasks()
    }

    pub fn selected_project(&self) -> Option<&ProjectRecord> {
        self.projects.get(self.selected_project)
    }

    pub fn selected_task(&self) -> Option<&TaskRecord> {
        self.tasks.get(self.selected_task)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<Option<String>> {
        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return Ok(None);
        }

        match self.route {
            Route::Home => ui::home::handler::handle(self, key),
            Route::ProjectDetail => ui::project_detail::handler::handle(self, key),
            Route::TaskDetail => ui::task::handler::handle(self, key),
            Route::AddRepo => ui::add_repo::handler::handle(self, key),
            Route::LiveSession => ui::live_session::handler::handle(self, key),
        }
    }

    pub fn latest_session_for_selected_project(&self) -> Option<&SessionRecord> {
        let project = self.selected_project()?;
        self.sessions
            .iter()
            .filter(|record| record.project_id == project.id)
            .max_by_key(|record| record.session.updated_at)
    }

    pub fn attach_selected_project_background_session(&mut self) -> Option<String> {
        let project = self.selected_project()?;
        let session_name = self
            .sessions
            .iter()
            .filter(|record| {
                record.project_id == project.id
                    && record.session.session_kind == SessionKind::Background
                    && !matches!(record.session.state, SessionState::Exited)
            })
            .max_by_key(|record| record.session.updated_at)
            .map(|record| {
                (
                    record.session.session_id.clone(),
                    record.session.tmux_session_name.clone(),
                )
            })?;
        self.route = Route::LiveSession;
        self.status = format!("Session {}", session_name.0);
        Some(session_name.1)
    }

    pub fn reload_tasks(&mut self) -> Result<()> {
        self.tasks = self
            .selected_project()
            .map(|project| self.services.task_store.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        if self.selected_task >= self.tasks.len() && !self.tasks.is_empty() {
            self.selected_task = self.tasks.len() - 1;
        }
        Ok(())
    }

    pub fn create_task(&mut self, description: impl Into<String>) -> Result<()> {
        let project = self
            .selected_project()
            .cloned()
            .ok_or_else(|| anyhow!("no project selected"))?;
        let description = description.into();
        let title = self
            .services
            .session_review_service
            .classify_task_title(&description);
        self.services
            .task_store
            .create_task(&project, title.clone(), description)?;
        self.reload_tasks()?;
        self.creating_task = false;
        self.task_draft.clear();
        self.status = format!("Task created: {title}");
        Ok(())
    }

    pub fn begin_task_creation(&mut self) {
        self.creating_task = true;
        self.task_draft.clear();
        self.status = "Enter a task description and press Enter".to_string();
    }

    pub fn cancel_task_creation(&mut self) {
        self.creating_task = false;
        self.task_draft.clear();
        self.status = "Task creation cancelled".to_string();
    }

    pub fn reset_add_repo_form(&mut self) {
        self.add_repo_form = AppState::new(&self.services.config).add_repo_form;
    }

    pub fn cycle_task_status(&mut self) -> Result<()> {
        let project = self
            .selected_project()
            .cloned()
            .ok_or_else(|| anyhow!("no project selected"))?;
        let task = self
            .selected_task()
            .cloned()
            .ok_or_else(|| anyhow!("no task selected"))?;
        let next = match task.status {
            TaskStatus::Todo => TaskStatus::InProgress,
            TaskStatus::InProgress => TaskStatus::Complete,
            TaskStatus::Complete => TaskStatus::WontDo,
            TaskStatus::WontDo => TaskStatus::Todo,
        };
        self.services
            .task_store
            .update_status(&project, &task.id, next)?;
        self.reload_tasks()?;
        self.status = "Task status updated".to_string();
        Ok(())
    }

    pub fn start_session(
        &mut self,
        product: CodingAgentProduct,
        kind: SessionKind,
    ) -> Result<String> {
        let project = self
            .selected_project()
            .cloned()
            .ok_or_else(|| anyhow!("no project selected"))?;
        let task = self
            .selected_task()
            .cloned()
            .ok_or_else(|| anyhow!("no task selected"))?;
        let session = self
            .services
            .session_service
            .start_session(&project, &task, product, kind)?;
        self.sessions = self
            .services
            .session_service
            .load_sessions()
            .unwrap_or_default();
        self.reload_tasks()?;
        self.route = Route::LiveSession;
        self.status = format!("Session {}", session.session_id);
        Ok(session.tmux_session_name)
    }

    pub fn attach_existing_session(
        &mut self,
        product: CodingAgentProduct,
        kind: SessionKind,
    ) -> Result<Option<String>> {
        let task = self
            .selected_task()
            .cloned()
            .ok_or_else(|| anyhow!("no task selected"))?;
        let session = task
            .sessions
            .into_iter()
            .find(|session| session.product == product && session.session_kind == kind);
        if let Some(session) = session {
            self.route = Route::LiveSession;
            self.status = format!("Session {}", session.session_id);
            Ok(Some(session.tmux_session_name))
        } else {
            self.status = format!("No {} {} session to attach", product.label(), kind.label());
            Ok(None)
        }
    }

    pub fn toggle_selected_project_auto_merge(&mut self) -> Result<()> {
        let project = self
            .selected_project()
            .cloned()
            .ok_or_else(|| anyhow!("no project selected"))?;
        let next = !project.config.auto_merge;
        self.services
            .project_catalog
            .update_project_config(&project.id, next)?;
        self.projects = self.services.project_catalog.load()?;
        self.status = if next {
            "Project set to auto-merge".to_string()
        } else {
            "Project set to open PRs".to_string()
        };
        Ok(())
    }
}
