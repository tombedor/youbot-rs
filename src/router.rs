use crate::models::{CommandRecord, RepoRecord, RepoStatus, RouteAction, RouteDecision};

pub fn route_message(
    message: &str,
    repos: &[RepoRecord],
    active_repo: Option<usize>,
) -> Option<RouteDecision> {
    let normalized = message.to_lowercase();
    let tokens = tokenize(&normalized);

    let repo_index = choose_repo(&normalized, &tokens, repos, active_repo)?;
    let repo = repos.get(repo_index)?;
    if repo.status != RepoStatus::Ready {
        return None;
    }

    let (command_name, args, reasoning) = choose_command(&normalized, &tokens, repo)?;
    Some(RouteDecision {
        action: if command_name == "__clarify__" {
            RouteAction::Clarify
        } else if command_name == "__adapter_change__" {
            RouteAction::AdapterChange
        } else if command_name == "__code_change__" {
            RouteAction::CodeChange
        } else {
            RouteAction::Command
        },
        repo_index,
        command_name,
        args,
        reasoning,
        prompt: if looks_like_code_change(&normalized) {
            Some(message.to_string())
        } else {
            None
        },
    })
}

fn choose_repo(
    normalized: &str,
    tokens: &[String],
    repos: &[RepoRecord],
    active_repo: Option<usize>,
) -> Option<usize> {
    let mut best: Option<(usize, i32)> = None;

    for (index, repo) in repos.iter().enumerate() {
        if repo.status != RepoStatus::Ready {
            continue;
        }

        let mut score = 0;
        if Some(index) == active_repo {
            score += 6;
        }

        for part in tokenize(&repo.name.to_lowercase()) {
            if tokens.iter().any(|token| token == &part) {
                score += 4;
            }
        }

        score += repo_domain_score(normalized, &repo.repo_id, &repo.name);

        for command in &repo.commands {
            score += command_match_score(normalized, tokens, command) / 2;
        }

        if score > 0 {
            match best {
                Some((_, best_score)) if score <= best_score => {}
                _ => best = Some((index, score)),
            }
        }
    }

    best.map(|(index, _)| index)
}

fn choose_command(
    normalized: &str,
    tokens: &[String],
    repo: &RepoRecord,
) -> Option<(String, Vec<String>, String)> {
    if looks_like_ambiguous_change(normalized) {
        return Some((
            "__clarify__".to_string(),
            Vec::new(),
            format!(
                "ambiguous repo-versus-adapter change target for {}",
                repo.name
            ),
        ));
    }
    if looks_like_adapter_change(normalized) {
        return Some((
            "__adapter_change__".to_string(),
            Vec::new(),
            format!("matched adapter/view change intent for {}", repo.name),
        ));
    }
    if repo.repo_id == "life_admin" {
        if contains_all(tokens, &["task", "list"])
            || contains_all(tokens, &["top", "task"])
            || contains_all(tokens, &["top", "tasks"])
        {
            let limit = extract_number(tokens).unwrap_or(5).to_string();
            return Some((
                "task-list".to_string(),
                vec![limit.clone(), "json".to_string()],
                format!(
                    "matched task listing intent for {} with limit {limit}",
                    repo.name
                ),
            ));
        }
        if contains_any(tokens, &["digest", "summary"]) && contains_any(tokens, &["task", "tasks"])
        {
            return Some((
                "task-digest".to_string(),
                vec!["json".to_string()],
                format!("matched task summary intent for {}", repo.name),
            ));
        }
        if contains_any(tokens, &["today", "agenda", "calendar"]) {
            return Some((
                "cal-today".to_string(),
                Vec::new(),
                format!("matched calendar agenda intent for {}", repo.name),
            ));
        }
    }

    if repo.repo_id == "job_search" {
        if contains_any(tokens, &["pipeline", "status"]) {
            return Some((
                "pipeline-status".to_string(),
                Vec::new(),
                format!("matched pipeline status intent for {}", repo.name),
            ));
        }
        if contains_any(tokens, &["opening", "openings", "jobs", "roles"]) {
            return Some((
                "active-openings".to_string(),
                Vec::new(),
                format!("matched active openings intent for {}", repo.name),
            ));
        }
        if contains_all(tokens, &["next", "action"]) || normalized.contains("next 7 days") {
            return Some((
                "next-actions".to_string(),
                Vec::new(),
                format!("matched next actions intent for {}", repo.name),
            ));
        }
    }

    if repo.repo_id == "trader-bot" {
        if contains_all(tokens, &["research", "program"]) {
            return Some((
                "research-program".to_string(),
                Vec::new(),
                format!("matched research program intent for {}", repo.name),
            ));
        }
        if contains_any(tokens, &["findings", "strategies", "strategy"]) {
            return Some((
                "research-findings".to_string(),
                Vec::new(),
                format!("matched research findings intent for {}", repo.name),
            ));
        }
        if contains_any(tokens, &["dataset", "datasets"]) {
            return Some((
                "list-datasets".to_string(),
                Vec::new(),
                format!("matched dataset listing intent for {}", repo.name),
            ));
        }
    }

    let best = repo
        .commands
        .iter()
        .map(|command| (command, command_match_score(normalized, tokens, command)))
        .max_by_key(|(_, score)| *score)?;

    if best.1 <= 0 {
        if looks_like_code_change(normalized) {
            return Some((
                "__code_change__".to_string(),
                Vec::new(),
                format!("matched code change intent for {}", repo.name),
            ));
        }
        return Some((
            "__clarify__".to_string(),
            Vec::new(),
            format!("low confidence route for {}", repo.name),
        ));
    }

    Some((
        best.0.command_name.clone(),
        Vec::new(),
        format!(
            "matched command `{}` from command metadata for {}",
            best.0.command_name, repo.name
        ),
    ))
}

fn repo_domain_score(normalized: &str, repo_id: &str, repo_name: &str) -> i32 {
    match repo_id {
        "job_search" if contains_any_str(normalized, &["job", "jobs", "pipeline", "opening"]) => 8,
        "life_admin" if contains_any_str(normalized, &["task", "tasks", "calendar", "agenda"]) => 8,
        "trader-bot"
            if contains_any_str(
                normalized,
                &["research", "market", "trading", "dataset", "strategy"],
            ) =>
        {
            8
        }
        _ if normalized.contains(&repo_name.to_lowercase()) => 4,
        _ => 0,
    }
}

fn command_match_score(normalized: &str, tokens: &[String], command: &CommandRecord) -> i32 {
    let mut score = 0;
    for word in tokenize(&command.command_name.to_lowercase()) {
        if tokens.iter().any(|token| token == &word) {
            score += 4;
        }
    }
    if let Some(description) = &command.description {
        for word in tokenize(&description.to_lowercase()) {
            if tokens.iter().any(|token| token == &word) {
                score += 1;
            }
        }
        if normalized.contains(&description.to_lowercase()) {
            score += 2;
        }
    }
    score
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn contains_any(tokens: &[String], expected: &[&str]) -> bool {
    expected
        .iter()
        .any(|needle| tokens.iter().any(|token| token == needle))
}

fn contains_all(tokens: &[String], expected: &[&str]) -> bool {
    expected
        .iter()
        .all(|needle| tokens.iter().any(|token| token == needle))
}

fn contains_any_str(haystack: &str, expected: &[&str]) -> bool {
    expected.iter().any(|needle| haystack.contains(needle))
}

fn extract_number(tokens: &[String]) -> Option<usize> {
    tokens.iter().find_map(|token| token.parse::<usize>().ok())
}

fn looks_like_code_change(normalized: &str) -> bool {
    [
        "change",
        "update",
        "edit",
        "fix",
        "implement",
        "add ",
        "remove",
        "refactor",
        "rewrite",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn looks_like_adapter_change(normalized: &str) -> bool {
    (looks_like_code_change(normalized)
        || ["show", "include", "add", "remove", "hide", "pin"]
            .iter()
            .any(|needle| normalized.contains(needle)))
        && [
            "in youbot",
            "inside youbot",
            "in the tui",
            "display",
            "shown",
            "show up",
            "view",
            "panel",
            "overview",
            "dashboard",
            "quick action",
            "shortcut",
            "workspace",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
}

fn looks_like_ambiguous_change(normalized: &str) -> bool {
    looks_like_code_change(normalized)
        && [
            "output",
            "results",
            "presentation",
            "format",
            "layout",
            "render",
        ]
        .iter()
        .any(|needle| normalized.contains(needle))
        && !looks_like_adapter_change(normalized)
}

#[cfg(test)]
mod tests {
    use super::route_message;
    use crate::models::{
        CommandRecord, RepoClassification, RepoRecord, RepoStatus, RouteAction,
        StructuredOutputFormat,
    };
    use std::path::PathBuf;

    #[test]
    fn routes_pipeline_queries_to_job_search() {
        let repos = sample_repos();
        let decision =
            route_message("what's my current job pipeline status?", &repos, None).unwrap();
        assert_eq!(repos[decision.repo_index].repo_id, "job_search");
        assert_eq!(decision.command_name, "pipeline-status");
    }

    #[test]
    fn routes_top_tasks_with_limit() {
        let repos = sample_repos();
        let decision = route_message("show me my top 5 tasks", &repos, None).unwrap();
        assert_eq!(repos[decision.repo_index].repo_id, "life_admin");
        assert_eq!(decision.command_name, "task-list");
        assert_eq!(decision.args, vec!["5".to_string(), "json".to_string()]);
    }

    #[test]
    fn active_repo_biases_selection() {
        let repos = sample_repos();
        let decision = route_message("show me the research program", &repos, Some(2)).unwrap();
        assert_eq!(repos[decision.repo_index].repo_id, "trader-bot");
        assert_eq!(decision.command_name, "research-program");
    }

    #[test]
    fn routes_view_changes_to_adapter_layer() {
        let repos = sample_repos();
        let decision = route_message(
            "show task-digest in the overview for life_admin inside youbot",
            &repos,
            None,
        )
        .unwrap();
        assert_eq!(decision.action, RouteAction::AdapterChange);
    }

    #[test]
    fn ambiguous_presentation_changes_require_clarification() {
        let repos = sample_repos();
        let decision = route_message("change the task output format", &repos, Some(1)).unwrap();
        assert_eq!(decision.action, RouteAction::Clarify);
    }

    fn sample_repos() -> Vec<RepoRecord> {
        vec![
            repo(
                "job_search",
                vec!["pipeline-status", "active-openings", "next-actions"],
            ),
            repo("life_admin", vec!["task-list", "task-digest", "cal-today"]),
            repo(
                "trader-bot",
                vec!["research-program", "research-findings", "list-datasets"],
            ),
        ]
    }

    fn repo(repo_id: &str, commands: Vec<&str>) -> RepoRecord {
        RepoRecord {
            repo_id: repo_id.to_string(),
            name: repo_id.to_string(),
            path: PathBuf::from(format!("/tmp/{repo_id}")),
            classification: RepoClassification::Integrated,
            status: RepoStatus::Ready,
            purpose_summary: None,
            tags: Vec::new(),
            preferred_commands: Vec::new(),
            commands: commands
                .into_iter()
                .map(|command_name| CommandRecord {
                    repo_id: repo_id.to_string(),
                    command_name: command_name.to_string(),
                    display_name: command_name.replace('-', " "),
                    description: None,
                    invocation: vec!["just".to_string(), command_name.to_string()],
                    supports_structured_output: false,
                    structured_output_format: StructuredOutputFormat::Unknown,
                    tags: Vec::new(),
                })
                .collect(),
            last_scanned_at: None,
            last_active_at: None,
            adapter_id: Some(format!("{repo_id}-adapter")),
            preferred_backend: None,
        }
    }
}
