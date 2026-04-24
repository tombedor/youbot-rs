use crate::models::{ProjectConfig, ProjectRecord};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProjectRegistry {
    state_root: PathBuf,
}

impl ProjectRegistry {
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
        }
    }

    fn registry_path(&self) -> PathBuf {
        self.state_root.join("projects.json")
    }

    pub fn load(&self) -> Result<Vec<ProjectRecord>> {
        let path = self.registry_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let projects = serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(projects)
    }

    pub fn save(&self, projects: &[ProjectRecord]) -> Result<()> {
        fs::create_dir_all(&self.state_root)
            .with_context(|| format!("failed to create {}", self.state_root.display()))?;
        let body = serde_json::to_string_pretty(projects)?;
        let path = self.registry_path();
        fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
        self.commit_state_repo("Update project registry")
    }

    pub fn add_existing_repo(
        &self,
        path: impl Into<PathBuf>,
        auto_merge: bool,
    ) -> Result<ProjectRecord> {
        let path = path.into();
        if !path.exists() {
            return Err(anyhow!("repo path does not exist: {}", path.display()));
        }
        let mut projects = self.load()?;
        let name = infer_name(&path);
        let record = ProjectRecord {
            id: Uuid::new_v4().to_string(),
            name,
            path,
            created_at: Utc::now(),
            config: ProjectConfig { auto_merge },
        };
        projects.push(record.clone());
        self.save(&projects)?;
        Ok(record)
    }

    pub fn create_new_repo(
        &self,
        root: &Path,
        name: &str,
        programming_language: &str,
        auto_merge: bool,
    ) -> Result<ProjectRecord> {
        let repo_path = root.join(name);
        fs::create_dir_all(&repo_path)
            .with_context(|| format!("failed to create {}", repo_path.display()))?;
        ensure_git_repo(&repo_path)?;
        write_gitignore(&repo_path, programming_language)?;
        self.add_existing_repo(repo_path, auto_merge)
    }

    pub fn update_project_config(&self, project_id: &str, auto_merge: bool) -> Result<()> {
        let mut projects = self.load()?;
        let project = projects
            .iter_mut()
            .find(|project| project.id == project_id)
            .ok_or_else(|| anyhow!("unknown project id {project_id}"))?;
        project.config.auto_merge = auto_merge;
        self.save(&projects)
    }

    fn commit_state_repo(&self, message: &str) -> Result<()> {
        let git_dir = self.state_root.join(".git");
        if !git_dir.exists() {
            let status = Command::new("git")
                .arg("init")
                .current_dir(&self.state_root)
                .status()
                .with_context(|| format!("failed to initialize git repo in {}", self.state_root.display()))?;
            if !status.success() {
                return Err(anyhow!("git init failed for {}", self.state_root.display()));
            }
        }

        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&self.state_root)
            .status()
            .with_context(|| format!("failed to git add in {}", self.state_root.display()))?;
        if !add_status.success() {
            return Err(anyhow!("git add failed in {}", self.state_root.display()));
        }

        let commit_status = Command::new("git")
            .args(["commit", "-m", message, "--allow-empty"])
            .current_dir(&self.state_root)
            .status()
            .with_context(|| format!("failed to git commit in {}", self.state_root.display()))?;
        if !commit_status.success() {
            return Err(anyhow!("git commit failed in {}", self.state_root.display()));
        }
        Ok(())
    }
}

fn infer_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string())
}

fn ensure_git_repo(path: &Path) -> Result<()> {
    if path.join(".git").exists() {
        return Ok(());
    }
    let status = Command::new("git")
        .arg("init")
        .current_dir(path)
        .status()
        .with_context(|| format!("failed to initialize git repo in {}", path.display()))?;
    if !status.success() {
        return Err(anyhow!("git init failed in {}", path.display()));
    }
    Ok(())
}

fn write_gitignore(path: &Path, programming_language: &str) -> Result<()> {
    let body = match programming_language.to_ascii_lowercase().as_str() {
        "rust" => "target/\nCargo.lock\n",
        "node" | "javascript" | "typescript" => "node_modules/\ndist/\n.env\n",
        "python" => "__pycache__/\n.venv/\n.pytest_cache/\n",
        _ => "",
    };
    if body.is_empty() {
        return Ok(());
    }
    fs::write(path.join(".gitignore"), body)
        .with_context(|| format!("failed to write {}", path.join(".gitignore").display()))?;
    Ok(())
}
