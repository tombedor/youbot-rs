use crate::application::session_review_service::SessionReviewService;
use crate::application::session_service::SessionService;
use crate::config;
use crate::domain::AppConfig;
use crate::infrastructure::notification::SystemNotifier;
use crate::infrastructure::project_catalog::ProjectCatalog;
use crate::infrastructure::task_store::TaskStore;
use crate::infrastructure::tmux::TmuxTerminal;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct AppServices {
    pub config: AppConfig,
    pub project_catalog: ProjectCatalog,
    pub task_store: TaskStore,
    pub session_review_service: SessionReviewService,
    pub session_service: SessionService,
}

impl AppServices {
    pub fn load() -> Result<Self> {
        let config = config::load_or_create()?;
        let project_catalog = ProjectCatalog::new(config.state_root.clone());
        let task_store = TaskStore::new(config.state_root.clone(), project_catalog.clone());
        let session_review_service = SessionReviewService::new(task_store.clone());
        let session_service = SessionService::new(
            config.state_root.clone(),
            config.monitor_silence_seconds,
            TmuxTerminal::new(config.tmux_socket_name.clone()),
            session_review_service.clone(),
            SystemNotifier,
            task_store.clone(),
            project_catalog.clone(),
        );
        Ok(Self {
            config,
            project_catalog,
            task_store,
            session_review_service,
            session_service,
        })
    }
}
