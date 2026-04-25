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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AgentSessionRef, ProjectConfig, ProjectRecord, SessionState, TaskRecord,
    };
    use crate::notifier::NotifySink;
    use crate::tmux_client::TmuxOps;
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use std::path::Path;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn latest_session_prefers_most_recent_for_selected_project() {
        let mut app = test_app();
        let project_id = app.projects[0].id.clone();
        app.sessions = vec![
            session_record(&project_id, "task-1", "older", SessionKind::Background, SessionState::Active, Utc::now()),
            session_record(
                &project_id,
                "task-2",
                "newer",
                SessionKind::Live,
                SessionState::Completed,
                Utc::now() + Duration::seconds(5),
            ),
        ];

        let latest = app.latest_session_for_selected_project().unwrap();

        assert_eq!(latest.task_title, "newer");
    }

    #[test]
    fn attach_selected_project_background_session_skips_exited_sessions() {
        let mut app = test_app();
        let project_id = app.projects[0].id.clone();
        app.sessions = vec![
            session_record(
                &project_id,
                "task-1",
                "exited",
                SessionKind::Background,
                SessionState::Exited,
                Utc::now(),
            ),
            session_record(
                &project_id,
                "task-2",
                "active",
                SessionKind::Background,
                SessionState::Active,
                Utc::now() + Duration::seconds(5),
            ),
        ];

        let session_name = app.attach_selected_project_background_session().unwrap();

        assert_eq!(session_name, "tmux-task-2");
        assert_eq!(app.route, Route::LiveSession);
    }

    #[test]
    fn attach_existing_session_returns_none_when_missing() {
        let mut app = test_app();
        app.tasks = vec![TaskRecord {
            id: "task-1".to_string(),
            title: "Task".to_string(),
            description: "desc".to_string(),
            status: TaskStatus::Todo,
            sessions: Vec::new(),
        }];

        let result = app
            .attach_existing_session(CodingAgentProduct::Codex, SessionKind::Background)
            .unwrap();

        assert!(result.is_none());
        assert!(app.status.contains("No codex background session"));
    }

    #[test]
    fn reload_tasks_clamps_selection_to_last_task() {
        let mut app = test_app();
        let project = app.projects[0].clone();
        app.task_repository
            .create_task(&project, "One", "first")
            .unwrap();
        app.task_repository
            .create_task(&project, "Two", "second")
            .unwrap();
        app.reload_tasks().unwrap();
        app.selected_task = 5;

        app.task_repository
            .update_status(&project, &app.tasks[1].id, TaskStatus::Complete)
            .unwrap();
        app.reload_tasks().unwrap();

        assert_eq!(app.selected_task, 1);
    }

    fn test_app() -> App {
        let temp = tempdir().unwrap();
        let state_root = temp.path().join(".youbot");
        let config = AppConfig {
            state_root: state_root.clone(),
            managed_repo_root: temp.path().join("managed"),
            tmux_socket_name: "youbot-test".to_string(),
            monitor_silence_seconds: 120,
        };
        let project_registry = ProjectRegistry::new(state_root.clone());
        let task_repository = TaskRepository::new(state_root.clone(), project_registry.clone());
        let supervisor = CodingAgentSupervisor::new(task_repository.clone());
        let session_manager = SessionManager::with_handles(
            state_root.clone(),
            120,
            Arc::new(NoopTmux),
            supervisor.clone(),
            Arc::new(NoopNotifier),
            task_repository.clone(),
            project_registry.clone(),
        );
        let project = ProjectRecord {
            id: "project-1".to_string(),
            name: "example".to_string(),
            path: temp.path().join("repo"),
            created_at: Utc::now(),
            config: ProjectConfig::default(),
        };
        std::fs::create_dir_all(&project.path).unwrap();
        project_registry.save(std::slice::from_ref(&project)).unwrap();

        App {
            config: config.clone(),
            route: Route::Home,
            projects: vec![project],
            tasks: Vec::new(),
            selected_project: 0,
            selected_task: 0,
            add_repo_form: new_add_repo_form(&config),
            creating_task: false,
            task_draft: String::new(),
            status: "Ready".to_string(),
            should_quit: false,
            sessions: Vec::new(),
            supervisor,
            project_registry,
            task_repository,
            session_manager,
        }
    }

    fn session_record(
        project_id: &str,
        task_id: &str,
        task_title: &str,
        session_kind: SessionKind,
        state: SessionState,
        updated_at: chrono::DateTime<Utc>,
    ) -> SessionRecord {
        SessionRecord {
            project_id: project_id.to_string(),
            task_id: task_id.to_string(),
            task_title: task_title.to_string(),
            session: AgentSessionRef {
                product: CodingAgentProduct::Codex,
                session_kind,
                tmux_session_name: format!("tmux-{task_id}"),
                session_id: format!("session-{task_id}"),
                state,
                branch_name: None,
                last_summary: None,
                created_at: updated_at,
                updated_at,
            },
        }
    }

    struct NoopTmux;

    impl TmuxOps for NoopTmux {
        fn session_exists(&self, _session_name: &str) -> bool {
            false
        }

        fn create_session(
            &self,
            _session_name: &str,
            _cwd: &Path,
            _command: &str,
            _detached: bool,
        ) -> Result<()> {
            Ok(())
        }

        fn attach(&self, _session_name: &str) -> Result<()> {
            Ok(())
        }

        fn capture_pane(&self, _session_name: &str) -> Result<String> {
            Ok(String::new())
        }

        fn send_keys(&self, _session_name: &str, _input: &str) -> Result<()> {
            Ok(())
        }

        fn enable_monitor_silence(&self, _session_name: &str, _seconds: u64) -> Result<()> {
            Ok(())
        }
    }

    struct NoopNotifier;

    impl NotifySink for NoopNotifier {
        fn notify(&self, _title: &str, _body: &str) -> Result<()> {
            Ok(())
        }
    }
}
