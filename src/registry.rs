use crate::justfile_parser;
use crate::models::{AppConfig, RepoRecord, RepoStatus};
use crate::persistence;
use anyhow::Result;
use chrono::Utc;

pub fn load(config: &AppConfig) -> Result<Vec<RepoRecord>> {
    let mut repos = Vec::new();

    for repo in &config.repos {
        let repo_path = &repo.path;
        let justfile = repo_path.join("justfile");

        let (status, commands) = if !repo_path.exists() {
            (RepoStatus::Missing, Vec::new())
        } else if !justfile.exists() {
            (RepoStatus::Invalid, Vec::new())
        } else {
            match justfile_parser::parse(&repo.repo_id, repo_path) {
                Ok(commands) => (RepoStatus::Ready, commands),
                Err(_) => (RepoStatus::Error, Vec::new()),
            }
        };

        let record = RepoRecord {
            repo_id: repo.repo_id.clone(),
            name: repo.name.clone(),
            path: repo.path.clone(),
            classification: repo.classification.clone(),
            status,
            purpose_summary: None,
            tags: Vec::new(),
            preferred_commands: Vec::new(),
            commands,
            last_scanned_at: Some(Utc::now().to_rfc3339()),
            last_active_at: None,
            adapter_id: Some(format!("{}-adapter", repo.repo_id)),
            preferred_backend: None,
        };
        let _ = persistence::ensure_adapter_metadata(&record);
        let _ = persistence::write_generated_adapter_note(
            &record.repo_id,
            &format!(
                "# {}\n\nGenerated adapter scaffold for {}.\n\nQuick actions are derived from discovered just commands.\n",
                record.name, record.name
            ),
        );
        repos.push(record);
    }

    let _ = persistence::persist_registry(&repos);
    Ok(repos)
}
