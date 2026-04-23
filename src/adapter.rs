use crate::models::{AdapterRecord, OverviewSectionSpec, QuickActionSpec, RepoRecord};
use crate::persistence;
use anyhow::Result;
use chrono::Utc;

pub fn load(repo: &RepoRecord) -> Result<AdapterRecord> {
    if let Some(adapter) = persistence::load_adapter_metadata(&repo.repo_id)? {
        Ok(adapter)
    } else {
        persistence::ensure_adapter_metadata(repo)
    }
}

pub fn apply_change(repo: &RepoRecord, request: &str) -> Result<String> {
    let mut adapter = load(repo)?;
    let normalized = request.to_lowercase();
    let command = match find_command_name(repo, &normalized) {
        Some(command_name) => command_name,
        None => {
            anyhow::bail!(
                "No matching repo command was found in that adapter request. Mention an existing command like `{}`.",
                repo.commands
                    .first()
                    .map(|command| command.command_name.as_str())
                    .unwrap_or("task-list")
            )
        }
    };

    let targets_overview = contains_any(
        &normalized,
        &["overview", "panel", "workspace", "dashboard", "summary"],
    );
    let targets_quick_actions = contains_any(
        &normalized,
        &[
            "quick action",
            "quick-action",
            "shortcut",
            "shortcut list",
            "action",
        ],
    );
    let remove = contains_any(&normalized, &["remove", "hide", "delete"]);
    let add = contains_any(&normalized, &["add", "show", "include", "pin"]);

    if !targets_overview && !targets_quick_actions {
        anyhow::bail!(
            "That sounds like a repo-presentation change, but I couldn't tell whether you meant the overview or quick actions."
        );
    }
    if !remove && !add {
        anyhow::bail!("I couldn't tell whether to add or remove that adapter element.");
    }

    let mut changes = Vec::new();

    if targets_overview {
        if remove {
            let before = adapter.overview_sections.len();
            adapter
                .overview_sections
                .retain(|section| section.command_name != command);
            if adapter.overview_sections.len() != before {
                changes.push(format!("removed `{command}` from the overview"));
            }
        } else if !adapter
            .overview_sections
            .iter()
            .any(|section| section.command_name == command)
        {
            let title = repo
                .commands
                .iter()
                .find(|candidate| candidate.command_name == command)
                .map(|candidate| candidate.display_name.clone());
            adapter.overview_sections.push(OverviewSectionSpec {
                command_name: command.clone(),
                arguments: Vec::new(),
                title,
                max_lines: 8,
                fallback_command_names: Vec::new(),
                render_mode: "auto".to_string(),
            });
            changes.push(format!("added `{command}` to the overview"));
        }
    }

    if targets_quick_actions {
        if remove {
            let before = adapter.quick_actions.len();
            adapter
                .quick_actions
                .retain(|action| action.command_name != command);
            if adapter.quick_actions.len() != before {
                changes.push(format!("removed `{command}` from quick actions"));
            }
        } else if !adapter
            .quick_actions
            .iter()
            .any(|action| action.command_name == command)
        {
            let title = repo
                .commands
                .iter()
                .find(|candidate| candidate.command_name == command)
                .map(|candidate| candidate.display_name.clone());
            adapter.quick_actions.push(QuickActionSpec {
                command_name: command.clone(),
                title,
                arguments: Vec::new(),
            });
            changes.push(format!("added `{command}` to quick actions"));
        }
    }

    if changes.is_empty() {
        anyhow::bail!("That adapter change did not alter the current adapter state.");
    }

    adapter.updated_at = Utc::now().to_rfc3339();
    persistence::store_adapter_metadata(&adapter)?;
    persistence::write_generated_adapter_note(
        &repo.repo_id,
        &format!(
            "# {}\n\nUpdated adapter metadata at {}.\n\n- {}\n",
            repo.name,
            adapter.updated_at,
            changes.join("\n- ")
        ),
    )?;

    Ok(format!(
        "Updated the {} adapter: {}.",
        repo.name,
        changes.join("; ")
    ))
}

fn find_command_name(repo: &RepoRecord, normalized: &str) -> Option<String> {
    repo.commands
        .iter()
        .find(|command| {
            normalized.contains(&command.command_name.to_lowercase())
                || normalized.contains(&command.display_name.to_lowercase())
        })
        .map(|command| command.command_name.clone())
        .or_else(|| {
            let tokens = tokenize(normalized);
            repo.commands
                .iter()
                .filter_map(|command| {
                    let score = tokenize(&command.command_name.to_lowercase())
                        .into_iter()
                        .chain(tokenize(&command.display_name.to_lowercase()))
                        .filter(|token| tokens.iter().any(|candidate| candidate == token))
                        .count();
                    if score > 0 {
                        Some((command.command_name.clone(), score))
                    } else {
                        None
                    }
                })
                .max_by_key(|(_, score)| *score)
                .map(|(command_name, _)| command_name)
        })
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}
