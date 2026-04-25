use crate::domain::ProjectRecord;
use crate::infrastructure::project_catalog::ProjectCatalog;
use crate::infrastructure::state_history::StateHistory;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectService {
    project_catalog: ProjectCatalog,
    state_history: StateHistory,
}

impl ProjectService {
    pub fn new(project_catalog: ProjectCatalog, state_history: StateHistory) -> Self {
        Self {
            project_catalog,
            state_history,
        }
    }

    pub fn load_projects(&self) -> Result<Vec<ProjectRecord>> {
        self.project_catalog.load()
    }

    pub fn add_existing_repo(
        &self,
        path: impl Into<PathBuf>,
        auto_merge: bool,
    ) -> Result<ProjectRecord> {
        let project = self.project_catalog.add_existing_repo(path, auto_merge)?;
        self.state_history
            .commit_snapshot("Update project registry")?;
        Ok(project)
    }

    pub fn create_new_repo(
        &self,
        root: &Path,
        name: &str,
        programming_language: &str,
        auto_merge: bool,
        remote_mode: usize,
    ) -> Result<ProjectRecord> {
        let project = self.project_catalog.create_new_repo(
            root,
            name,
            programming_language,
            auto_merge,
            remote_mode,
        )?;
        self.state_history
            .commit_snapshot("Update project registry")?;
        Ok(project)
    }

    pub fn set_auto_merge(&self, project_id: &str, auto_merge: bool) -> Result<()> {
        self.project_catalog
            .update_project_config(project_id, auto_merge)?;
        self.state_history
            .commit_snapshot("Update project registry")
    }
}
