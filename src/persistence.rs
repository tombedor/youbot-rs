use crate::config::state_root;
use crate::models::{
    AdapterRecord, CodingAgentActivity, CodingAgentRunLog, CodingAgentSessionRef, CommandRunLog,
    ConversationRecord, QuickActionSpec, RepoRecord, ReviewBundle,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

pub fn persist_registry(repos: &[RepoRecord]) -> Result<()> {
    write_json(pretty_path("registry/repos.json")?, repos)?;

    let mut commands: BTreeMap<String, Value> = BTreeMap::new();
    for repo in repos {
        commands.insert(repo.repo_id.clone(), serde_json::to_value(&repo.commands)?);
    }
    write_json(pretty_path("registry/commands.json")?, &commands)?;
    Ok(())
}

pub fn ensure_adapter_metadata(repo: &RepoRecord) -> Result<AdapterRecord> {
    let path = pretty_path(&format!("adapters/metadata/{}.json", repo.repo_id))?;
    if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed reading {}", path.display()))?;
        let adapter: AdapterRecord = serde_json::from_str(&raw)
            .with_context(|| format!("failed parsing {}", path.display()))?;
        return Ok(adapter);
    }

    let adapter = AdapterRecord {
        adapter_id: format!("{}-adapter", repo.repo_id),
        repo_id: repo.repo_id.clone(),
        version: "0.1.0".to_string(),
        view_names: vec!["overview".to_string(), "conversation".to_string()],
        command_palette_entries: repo
            .commands
            .iter()
            .map(|command| command.command_name.clone())
            .collect(),
        output_rules: vec!["repo_overview_preview".to_string()],
        updated_at: Utc::now().to_rfc3339(),
        overview_sections: repo
            .commands
            .iter()
            .take(3)
            .map(|command| crate::models::OverviewSectionSpec {
                command_name: command.command_name.clone(),
                arguments: if command.supports_structured_output {
                    vec!["json".to_string()]
                } else {
                    Vec::new()
                },
                title: Some(command.display_name.clone()),
                max_lines: 8,
                fallback_command_names: Vec::new(),
                render_mode: if command.supports_structured_output {
                    "json".to_string()
                } else {
                    "auto".to_string()
                },
            })
            .collect(),
        quick_actions: repo
            .commands
            .iter()
            .take(4)
            .map(|command| QuickActionSpec {
                command_name: command.command_name.clone(),
                title: Some(command.display_name.clone()),
                arguments: if command.supports_structured_output {
                    vec!["json".to_string()]
                } else {
                    Vec::new()
                },
            })
            .collect(),
    };
    write_json(path, &adapter)?;
    Ok(adapter)
}

pub fn load_adapter_metadata(repo_id: &str) -> Result<Option<AdapterRecord>> {
    let path = pretty_path(&format!("adapters/metadata/{repo_id}.json"))?;
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed reading {}", path.display()))?;
    let adapter =
        serde_json::from_str(&raw).with_context(|| format!("failed parsing {}", path.display()))?;
    Ok(Some(adapter))
}

pub fn store_adapter_metadata(adapter: &AdapterRecord) -> Result<()> {
    write_json(
        pretty_path(&format!("adapters/metadata/{}.json", adapter.repo_id))?,
        adapter,
    )
}

pub fn append_command_run(log: &CommandRunLog) -> Result<()> {
    append_jsonl(pretty_path("runs/commands.jsonl")?, log)
}

pub fn append_coding_agent_run(log: &CodingAgentRunLog) -> Result<()> {
    append_jsonl(pretty_path("runs/coding_agents.jsonl")?, log)
}

pub fn load_command_runs(limit: usize) -> Result<Vec<Value>> {
    read_recent_jsonl(pretty_path("runs/commands.jsonl")?, limit)
}

pub fn load_coding_agent_runs(limit: usize) -> Result<Vec<Value>> {
    read_recent_jsonl(pretty_path("runs/coding_agents.jsonl")?, limit)
}

pub fn load_activity_entries(limit: usize) -> Result<Vec<Value>> {
    read_recent_jsonl(pretty_path("activity/coding_agent_events.jsonl")?, limit)
}

pub fn write_current_activity(activity: &CodingAgentActivity) -> Result<()> {
    write_json(pretty_path("activity/coding_agent_current.json")?, activity)
}

pub fn load_current_activity() -> Result<Option<CodingAgentActivity>> {
    let path = pretty_path("activity/coding_agent_current.json")?;
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed reading {}", path.display()))?;
    let activity =
        serde_json::from_str(&raw).with_context(|| format!("failed parsing {}", path.display()))?;
    Ok(Some(activity))
}

pub fn append_activity_event(event: &Value) -> Result<()> {
    append_jsonl(pretty_path("activity/coding_agent_events.jsonl")?, event)
}

pub fn load_sessions() -> Result<BTreeMap<String, CodingAgentSessionRef>> {
    let path = pretty_path("coding_agent_sessions/sessions.json")?;
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed reading {}", path.display()))?;
    let sessions =
        serde_json::from_str(&raw).with_context(|| format!("failed parsing {}", path.display()))?;
    Ok(sessions)
}

pub fn store_session(session: &CodingAgentSessionRef) -> Result<()> {
    let mut sessions = load_sessions()?;
    sessions.insert(session.repo_id.clone(), session.clone());
    write_json(
        pretty_path("coding_agent_sessions/sessions.json")?,
        &sessions,
    )
}

pub fn create_review_bundle(conversation: &ConversationRecord) -> Result<PathBuf> {
    let command_runs = load_command_runs(100)?;
    let coding_agent_runs = load_coding_agent_runs(100)?;
    let activity_entries = load_activity_entries(100)?;
    let bundle_id = Uuid::new_v4().to_string();
    let bundle_path = pretty_path(&format!("reviews/bundles/{bundle_id}.json"))?;

    let bundle = ReviewBundle {
        bundle_id: bundle_id.clone(),
        created_at: Utc::now().to_rfc3339(),
        source_state_root: state_root()?.display().to_string(),
        window_summary: format!(
            "{} messages, {} command runs, {} coding-agent runs",
            conversation.messages.len(),
            command_runs.len(),
            coding_agent_runs.len()
        ),
        conversation_id: Some(conversation.conversation_id.clone()),
        messages: conversation.messages.clone(),
        command_runs,
        coding_agent_runs,
        activity_entries,
        activity_log_refs: vec![
            pretty_path("runs/commands.jsonl")?.display().to_string(),
            pretty_path("runs/coding_agents.jsonl")?
                .display()
                .to_string(),
            pretty_path("activity/coding_agent_events.jsonl")?
                .display()
                .to_string(),
        ],
        notes: vec!["Generated by youbot-rs review-usage".to_string()],
    };

    write_json(&bundle_path, &bundle)?;
    write_json(
        pretty_path("reviews/latest.json")?,
        &serde_json::json!({
            "bundle_id": bundle.bundle_id,
            "created_at": bundle.created_at,
            "bundle_path": bundle_path.display().to_string(),
            "window_summary": bundle.window_summary,
        }),
    )?;
    Ok(bundle_path)
}

pub fn append_scheduler_run(entry: &Value) -> Result<()> {
    let path = pretty_path("scheduler/run_history.json")?;
    let mut runs: Vec<Value> = if path.exists() {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed reading {}", path.display()))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    };
    runs.push(entry.clone());
    write_json(path, &runs)
}

pub fn write_generated_adapter_note(repo_id: &str, body: &str) -> Result<()> {
    let path = pretty_path(&format!("adapters/generated/{repo_id}.md"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating {}", parent.display()))?;
    }
    fs::write(&path, body).with_context(|| format!("failed writing {}", path.display()))?;
    Ok(())
}

fn write_json(path: impl Into<PathBuf>, value: &(impl Serialize + ?Sized)) -> Result<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating {}", parent.display()))?;
    }
    let body = serde_json::to_string_pretty(value)?;
    fs::write(&path, body).with_context(|| format!("failed writing {}", path.display()))?;
    Ok(())
}

fn append_jsonl(path: impl Into<PathBuf>, value: &impl Serialize) -> Result<()> {
    let path = path.into();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed creating {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed opening {}", path.display()))?;
    writeln!(file, "{}", serde_json::to_string(value)?)
        .with_context(|| format!("failed writing {}", path.display()))?;
    Ok(())
}

fn read_recent_jsonl(path: impl Into<PathBuf>, limit: usize) -> Result<Vec<Value>> {
    let path = path.into();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed reading {}", path.display()))?;
    let mut values = raw
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    if values.len() > limit {
        values = values.split_off(values.len() - limit);
    }
    Ok(values)
}

fn pretty_path(rel: &str) -> Result<PathBuf> {
    Ok(state_root()?.join(rel))
}
