use crate::models::{CommandRunLog, ExecutionResult, RepoRecord};
use crate::persistence;
use anyhow::{Context, Result};
use chrono::Utc;
use std::process::Command;

pub fn run(repo: &RepoRecord, command_name: &str, args: &[String]) -> Result<ExecutionResult> {
    let started = Utc::now();
    let started_at = started.to_rfc3339();

    let output = Command::new("just")
        .arg(command_name)
        .args(args)
        .current_dir(&repo.path)
        .output()
        .with_context(|| {
            format!(
                "failed to run just {} in {}",
                command_name,
                repo.path.display()
            )
        })?;

    let finished = Utc::now();
    let finished_at = finished.to_rfc3339();

    let result = ExecutionResult {
        repo_id: repo.repo_id.clone(),
        command_name: command_name.to_string(),
        invocation: {
            let mut invocation = vec!["just".to_string(), command_name.to_string()];
            invocation.extend(args.iter().cloned());
            invocation
        },
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        started_at,
        finished_at,
        duration_ms: (finished - started).num_milliseconds(),
    };

    persistence::append_command_run(&CommandRunLog {
        repo_id: result.repo_id.clone(),
        command_name: result.command_name.clone(),
        invocation: result.invocation.clone(),
        exit_code: result.exit_code,
        started_at: result.started_at.clone(),
        finished_at: result.finished_at.clone(),
    })?;

    Ok(result)
}
