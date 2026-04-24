use crate::coding_agent_supervisor::CodingAgentSupervisor;
use crate::config;
use crate::controllers;
use crate::models::{
    AddRepoForm, AppConfig, CodingAgentProduct, ProjectRecord, Route, SessionKind, SessionRecord,
    TaskRecord, TaskStatus,
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
    pub status: String,
    pub should_quit: bool,
    pub sessions: Vec<SessionRecord>,
    pub project_registry: ProjectRegistry,
    pub task_repository: TaskRepository,
    pub session_manager: SessionManager,
}

impl App {
    pub fn load() -> Result<Self> {
        let config = config::load_or_create()?;
        let project_registry = ProjectRegistry::new(config.state_root.clone());
        let task_repository = TaskRepository::new(config.state_root.clone());
        let supervisor = CodingAgentSupervisor::new(task_repository.clone());
        let session_manager = SessionManager::new(
            config.state_root.clone(),
            config.monitor_silence_seconds,
            TmuxClient::new(config.tmux_socket_name.clone()),
            supervisor,
            Notifier,
            task_repository.clone(),
        );
        let projects = project_registry.load()?;
        let selected_project = 0;
        let tasks = projects
            .first()
            .map(|project| task_repository.load_tasks(project))
            .transpose()?
            .unwrap_or_default();
        let sessions = session_manager.load_sessions().unwrap_or_default();

        Ok(Self {
            config,
            route: Route::Home,
            projects,
            tasks,
            selected_project,
            selected_task: 0,
            add_repo_form: AddRepoForm::default(),
            status: "Ready".to_string(),
            should_quit: false,
            sessions,
            project_registry,
            task_repository,
            session_manager,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.projects = self.project_registry.load()?;
        self.sessions = self.session_manager.poll(&self.projects).unwrap_or_default();
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

    pub fn create_task(&mut self, title: impl Into<String>) -> Result<()> {
        let project = self
            .selected_project()
            .cloned()
            .ok_or_else(|| anyhow!("no project selected"))?;
        self.task_repository.create_task(&project, title)?;
        self.reload_tasks()?;
        self.status = "Task created".to_string();
        Ok(())
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
        self.task_repository.update_status(&project, &task.id, next)?;
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
}
