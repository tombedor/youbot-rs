use crate::adapter;
use crate::executor;
use crate::models::{
    ExecutionResult, OverviewCard, QuickActionView, RepoOverview, RepoRecord, RepoStatus,
};
use anyhow::Result;
use serde_json::Value;

pub fn build(repo: &RepoRecord) -> Result<RepoOverview> {
    let adapter = adapter::load(repo)?;
    let mut cards = Vec::new();

    for section in &adapter.overview_sections {
        let Some(command_name) =
            resolve_command_name(repo, &section.command_name, &section.fallback_command_names)
        else {
            continue;
        };
        let result = executor::run(repo, &command_name, &section.arguments)?;
        let title = section
            .title
            .clone()
            .or_else(|| {
                repo.commands
                    .iter()
                    .find(|command| command.command_name == command_name)
                    .map(|command| command.display_name.clone())
            })
            .unwrap_or_else(|| command_name.clone());
        let lines = render_section(
            repo,
            &command_name,
            &section.render_mode,
            &result,
            section.max_lines,
        );
        cards.push(OverviewCard { title, lines });
    }

    if cards.is_empty() {
        cards.push(OverviewCard {
            title: "Commands".to_string(),
            lines: repo
                .commands
                .iter()
                .take(8)
                .map(|command| {
                    command
                        .description
                        .as_ref()
                        .map(|description| format!("{}: {}", command.command_name, description))
                        .unwrap_or_else(|| command.command_name.clone())
                })
                .collect(),
        });
    }

    let quick_actions = build_quick_actions(repo, &adapter.quick_actions);
    Ok(RepoOverview {
        subtitle: format!("{} workspace", repo.name),
        cards,
        quick_actions,
    })
}

pub fn summarize_execution(repo: &RepoRecord, result: &ExecutionResult) -> String {
    match repo.repo_id.as_str() {
        "job_search" => summarize_job_search(result),
        "life_admin" => summarize_life_admin(result),
        "trader-bot" => summarize_trader_bot(result),
        _ => summarize_generic(result),
    }
}

fn resolve_command_name(
    repo: &RepoRecord,
    preferred: &str,
    fallbacks: &[String],
) -> Option<String> {
    if repo
        .commands
        .iter()
        .any(|command| command.command_name == preferred)
    {
        return Some(preferred.to_string());
    }
    fallbacks.iter().find_map(|fallback| {
        repo.commands
            .iter()
            .find(|command| command.command_name == *fallback)
            .map(|command| command.command_name.clone())
    })
}

fn build_quick_actions(
    repo: &RepoRecord,
    quick_actions: &[crate::models::QuickActionSpec],
) -> Vec<QuickActionView> {
    let mut items = quick_actions
        .iter()
        .filter(|action| {
            repo.commands
                .iter()
                .any(|command| command.command_name == action.command_name)
        })
        .map(|action| QuickActionView {
            title: action
                .title
                .clone()
                .unwrap_or_else(|| action.command_name.replace('-', " ")),
            command_name: action.command_name.clone(),
            arguments: action.arguments.clone(),
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        items = repo
            .commands
            .iter()
            .take(4)
            .map(|command| QuickActionView {
                title: command.display_name.clone(),
                command_name: command.command_name.clone(),
                arguments: Vec::new(),
            })
            .collect();
    }

    items
}

fn render_section(
    repo: &RepoRecord,
    _command_name: &str,
    render_mode: &str,
    result: &ExecutionResult,
    max_lines: usize,
) -> Vec<String> {
    if result.exit_code != 0 {
        return first_non_empty_lines(&result.stderr, max_lines.max(1));
    }

    let lines = match render_mode {
        "json" => summarize_json_payload(&result.stdout, max_lines),
        "bullets" => parse_bullets(&result.stdout),
        "auto" => summarize_by_command(repo, result),
        "text" => summarize_by_command(repo, result),
        _ => summarize_by_command(repo, result),
    };

    let mut lines = lines
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .take(max_lines.max(1))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push("No data returned.".to_string());
    }
    lines
}

fn summarize_by_command(repo: &RepoRecord, result: &ExecutionResult) -> Vec<String> {
    match repo.repo_id.as_str() {
        "job_search" => match result.command_name.as_str() {
            "pipeline-status" => summarize_pipeline_table(&result.stdout),
            "next-actions" | "active-openings" => parse_bullets(&result.stdout),
            _ => first_non_empty_lines(&result.stdout, 12),
        },
        "life_admin" => match result.command_name.as_str() {
            "task-digest" => summarize_task_digest(&result.stdout),
            "task-list" => summarize_task_list(&result.stdout),
            _ => first_non_empty_lines(&result.stdout, 12),
        },
        "trader-bot" => match result.command_name.as_str() {
            "research-program" => summarize_research_program(&result.stdout),
            "research-findings" => summarize_findings(&result.stdout),
            _ => first_non_empty_lines(&result.stdout, 12),
        },
        _ => summarize_json_payload(&result.stdout, 12),
    }
}

fn summarize_job_search(result: &ExecutionResult) -> String {
    match result.command_name.as_str() {
        "pipeline-status" => summarize_pipeline_table(&result.stdout).join("\n"),
        "next-actions" | "active-openings" => parse_bullets(&result.stdout).join("\n"),
        _ => summarize_generic(result),
    }
}

fn summarize_life_admin(result: &ExecutionResult) -> String {
    match result.command_name.as_str() {
        "task-digest" => summarize_task_digest(&result.stdout).join("\n"),
        "task-list" => summarize_task_list(&result.stdout).join("\n"),
        _ => summarize_generic(result),
    }
}

fn summarize_trader_bot(result: &ExecutionResult) -> String {
    match result.command_name.as_str() {
        "research-program" => summarize_research_program(&result.stdout).join("\n"),
        "research-findings" => summarize_findings(&result.stdout).join("\n"),
        _ => summarize_generic(result),
    }
}

fn summarize_generic(result: &ExecutionResult) -> String {
    if result.exit_code != 0 {
        let stderr = first_non_empty_lines(&result.stderr, 6);
        return format!(
            "Command failed with exit code {}.\n{}",
            result.exit_code,
            stderr.join("\n")
        );
    }

    first_non_empty_lines(&result.stdout, 12).join("\n")
}

fn summarize_pipeline_table(stdout: &str) -> Vec<String> {
    let mut total = 0usize;
    let mut in_progress = 0usize;
    let mut rejected = 0usize;
    let mut highlights = Vec::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || trimmed.contains("Company") || trimmed.contains("----") {
            continue;
        }
        let cells: Vec<_> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim())
            .collect();
        if cells.len() < 3 {
            continue;
        }
        total += 1;
        let company = cells[0];
        let status = cells[1];
        let notes = cells[2];
        if status.contains("Rejected") || status.contains("Soft No") || status.contains("Closed") {
            rejected += 1;
        } else {
            in_progress += 1;
            highlights.push(if notes.is_empty() {
                format!("{company}: {status}")
            } else {
                format!("{company}: {status} - {notes}")
            });
        }
    }

    let mut lines = vec![
        format!("total tracked: {total}"),
        format!("active or pending: {in_progress}"),
        format!("rejected or closed: {rejected}"),
    ];
    lines.extend(highlights.into_iter().take(5));
    lines
}

fn summarize_task_digest(stdout: &str) -> Vec<String> {
    let value = extract_json(stdout).unwrap_or(Value::Null);
    let counts = value.get("counts").and_then(Value::as_object);
    let urgent = counts
        .and_then(|counts| counts.get("urgent"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let high = counts
        .and_then(|counts| counts.get("high"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let medium = counts
        .and_then(|counts| counts.get("medium"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let low = counts
        .and_then(|counts| counts.get("low"))
        .and_then(Value::as_i64)
        .unwrap_or(0);

    vec![
        format!("urgent: {urgent}"),
        format!("high: {high}"),
        format!("medium: {medium}"),
        format!("low: {low}"),
    ]
}

fn summarize_task_list(stdout: &str) -> Vec<String> {
    let value = extract_json(stdout).unwrap_or(Value::Null);
    value
        .get("tasks")
        .and_then(Value::as_array)
        .map(|tasks| {
            tasks
                .iter()
                .take(5)
                .map(|task| {
                    let title = task
                        .get("title")
                        .and_then(Value::as_str)
                        .unwrap_or("untitled");
                    let priority = task
                        .get("priority")
                        .and_then(Value::as_str)
                        .unwrap_or("none");
                    let status = task
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    format!("{title} [{status}, {priority}]")
                })
                .collect()
        })
        .unwrap_or_else(|| vec!["No tasks available.".to_string()])
}

fn summarize_research_program(stdout: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## Research Goal") {
            continue;
        }
        if trimmed.starts_with("### ") {
            lines.push(trimmed.trim_start_matches("### ").to_string());
        } else if trimmed.starts_with("Target:") || trimmed.starts_with("Validation threshold:") {
            lines.push(trimmed.to_string());
        }
    }
    if lines.is_empty() {
        first_non_empty_lines(stdout, 8)
    } else {
        lines.into_iter().take(8).collect()
    }
}

fn summarize_findings(stdout: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_strategy: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") {
            current_strategy = Some(trimmed.trim_start_matches("### ").to_string());
        } else if trimmed.starts_with("- Hit rate:")
            && let Some(strategy) = current_strategy.take()
        {
            lines.push(format!("{strategy}: {}", trimmed.trim_start_matches("- ")));
        }
    }

    if lines.is_empty() {
        first_non_empty_lines(stdout, 8)
    } else {
        lines.into_iter().take(6).collect()
    }
}

fn summarize_json_payload(stdout: &str, max_lines: usize) -> Vec<String> {
    let value = match extract_json(stdout) {
        Some(value) => value,
        None => return first_non_empty_lines(stdout, max_lines),
    };

    match value {
        Value::Object(map) => map
            .into_iter()
            .take(max_lines)
            .map(|(key, value)| format!("{key}: {}", compact_json_value(&value)))
            .collect(),
        Value::Array(items) => items
            .into_iter()
            .take(max_lines)
            .map(|item| compact_json_value(&item))
            .collect(),
        other => vec![compact_json_value(&other)],
    }
}

fn compact_json_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => format!("{} items", items.len()),
        Value::Object(map) => map
            .iter()
            .take(3)
            .map(|(key, value)| format!("{key}={}", compact_json_value(value)))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn parse_bullets(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("- [ ] ")
                .or_else(|| trimmed.strip_prefix("- "))
                .map(ToString::to_string)
        })
        .take(8)
        .collect()
}

fn first_non_empty_lines(stdout: &str, limit: usize) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(limit)
        .map(ToString::to_string)
        .collect()
}

fn extract_json(stdout: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(stdout.trim()) {
        return Some(value);
    }
    let start = stdout.find('{')?;
    serde_json::from_str(&stdout[start..]).ok()
}

#[allow(dead_code)]
fn _ready(repo: &RepoRecord) -> bool {
    repo.status == RepoStatus::Ready
}
