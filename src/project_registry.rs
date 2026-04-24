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
        let path = normalize_repo_path(path.into())?;
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
            run_git(&self.state_root, ["init"])?;
        }

        run_git(&self.state_root, ["add", "."])?;
        run_git(&self.state_root, ["commit", "-m", message, "--allow-empty"])?;
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
    run_git(path, ["init"])?;
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

fn normalize_repo_path(path: PathBuf) -> Result<PathBuf> {
    let raw = path.to_string_lossy();
    if raw == "~" || raw.starts_with("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("failed to determine home directory"))?;
        let suffix = raw.strip_prefix("~/").unwrap_or("");
        return Ok(home.join(suffix));
    }
    Ok(path)
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to run git {:?} in {}", args, cwd.display()))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "git command failed with no output".to_string()
    };
    Err(anyhow!(
        "git {:?} failed in {}: {}",
        args,
        cwd.display(),
        details
    ))
}

#[cfg(test)]
mod tests {
    use super::normalize_repo_path;

    #[test]
    fn expands_tilde_paths() {
        let path = normalize_repo_path("~/development/example".into()).unwrap();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("/development/example"));
    }
}
