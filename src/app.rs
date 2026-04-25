use crate::coding_agent_supervisor::CodingAgentSupervisor;
use crate::config;
use crate::controllers;
use crate::models::{
    AddRepoField, AddRepoForm, AppConfig, CodingAgentProduct, ProjectRecord, Route, SessionKind,
    SessionRecord, TaskRecord, TaskStatus,
};
use crate::notifier::Notifier;
use crate::project_registry::ProjectRegistry;
use crate::session_manager::SessionManager;
use crate::task_repository::TaskRepository;
use crate::tmux_client::TmuxClient;
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent};

pub struct App {
    pub config: AppConfig,
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
    pub supervisor: CodingAgentSupervisor,
    pub project_registry: ProjectRegistry,
    pub task_repository: TaskRepository,
    pub session_manager: SessionManager,
}

impl App {
    pub fn load() -> Result<Self> {
        let config = config::load_or_create()?;
        let project_registry = ProjectRegistry::new(config.state_root.clone());
        let task_repository =
            TaskRepository::new(config.state_root.clone(), project_registry.clone());
        let supervisor = CodingAgentSupervisor::new(task_repository.clone());
        let session_manager = SessionManager::new(
            config.state_root.clone(),
            config.monitor_silence_seconds,
            TmuxClient::new(config.tmux_socket_name.clone()),
            supervisor.clone(),
            Notifier,
            task_repository.clone(),
            project_registry.clone(),
        );
        let projects = project_registry.load()?;
        let selected_project = 0;
        let tasks = projects
            .first()
            .map(|project| task_repository.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        let sessions = session_manager.load_sessions().unwrap_or_default();

        let add_repo_form = new_add_repo_form(&config);

        Ok(Self {
            config,
            route: Route::Home,
            projects,
            tasks,
            selected_project,
            selected_task: 0,
            add_repo_form,
            creating_task: false,
            task_draft: String::new(),
            status: "Ready".to_string(),
            should_quit: false,
            sessions,
            supervisor,
            project_registry,
            task_repository,
            session_manager,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.projects = self.project_registry.load()?;
        self.sessions = self
            .session_manager
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
            Route::Home => controllers::home::handle(self, key),
            Route::ProjectDetail => controllers::project_detail::handle(self, key),
            Route::TaskDetail => controllers::task::handle(self, key),
            Route::AddRepo => controllers::add_repo::handle(self, key),
            Route::LiveSession => controllers::live_session::handle(self, key),
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
        let session = self
            .sessions
            .iter()
            .filter(|record| {
                record.project_id == project.id
                    && record.session.session_kind == SessionKind::Background
                    && !matches!(record.session.state, crate::models::SessionState::Exited)
            })
            .max_by_key(|record| record.session.updated_at)?;
        self.route = Route::LiveSession;
        self.status = format!("Session {}", session.session.session_id);
        Some(session.session.tmux_session_name.clone())
    }

    pub fn reload_tasks(&mut self) -> Result<()> {
        self.tasks = self
            .selected_project()
            .map(|project| self.task_repository.load_tasks(project))
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
        let title = self.supervisor.classify_task_title(&description);
        self.task_repository
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
        self.add_repo_form = new_add_repo_form(&self.config);
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
        self.task_repository
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
            .session_manager
            .start_session(&project, &task, product, kind)?;
        self.sessions = self.session_manager.load_sessions().unwrap_or_default();
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
        self.project_registry
            .update_project_config(&project.id, next)?;
        self.projects = self.project_registry.load()?;
        self.status = if next {
            "Project set to auto-merge".to_string()
        } else {
            "Project set to open PRs".to_string()
        };
        Ok(())
    }
}

fn new_add_repo_form(config: &AppConfig) -> AddRepoForm {
    AddRepoForm {
        location_input: config.managed_repo_root.display().to_string(),
        programming_language: "rust".to_string(),
        active_field: AddRepoField::RepoInput,
        ..AddRepoForm::default()
    }
}
