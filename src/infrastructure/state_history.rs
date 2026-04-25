use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct StateHistory {
    state_root: PathBuf,
}

impl StateHistory {
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
        }
    }

    pub fn commit_snapshot(&self, message: &str) -> Result<()> {
        let git_dir = self.state_root.join(".git");
        if !git_dir.exists() {
            if let Err(error) = run_git(&self.state_root, ["init"]) {
                return Ok(log_commit_failure("git init", error));
            }
        }

        if let Err(error) = run_git(&self.state_root, ["add", "."]) {
            return Ok(log_commit_failure("git add", error));
        }
        if let Err(error) = run_git_commit(&self.state_root, message) {
            return Ok(log_commit_failure("git commit", error));
        }
        Ok(())
    }
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
    anyhow::bail!("git {:?} failed in {}: {}", args, cwd.display(), details)
}

fn run_git_commit(cwd: &Path, message: &str) -> Result<()> {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=youbot",
            "-c",
            "user.email=youbot@local",
            "commit",
            "-m",
            message,
            "--allow-empty",
        ])
        .current_dir(cwd)
        .output()
        .with_context(|| format!("failed to run git commit in {}", cwd.display()))?;
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
        "git commit failed with no output".to_string()
    };
    anyhow::bail!("git commit failed in {}: {}", cwd.display(), details)
}

fn log_commit_failure(operation: &str, error: anyhow::Error) {
    eprintln!("warning: state history {operation} failed: {error:#}");
}
