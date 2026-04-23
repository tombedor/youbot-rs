use crate::models::{CommandRecord, StructuredOutputFormat};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn parse(repo_id: &str, repo_root: &Path) -> Result<Vec<CommandRecord>> {
    let path = repo_root.join("justfile");
    let body = fs::read_to_string(&path)
        .with_context(|| format!("failed to read justfile {}", path.display()))?;

    let mut commands = Vec::new();
    let mut seen = HashSet::new();
    let mut pending_comment: Option<String> = None;

    for line in body.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            pending_comment = None;
            continue;
        }

        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }

        if let Some(comment) = trimmed.strip_prefix('#') {
            let comment = comment.trim();
            if !comment.is_empty() {
                pending_comment = Some(comment.to_string());
            }
            continue;
        }

        if is_non_recipe_line(trimmed) {
            pending_comment = None;
            continue;
        }

        let Some((head, _tail)) = trimmed.split_once(':') else {
            pending_comment = None;
            continue;
        };

        let command_name = head.split_whitespace().next().unwrap_or_default().trim();
        if command_name.is_empty()
            || !is_valid_recipe_name(command_name)
            || !seen.insert(command_name.to_string())
        {
            pending_comment = None;
            continue;
        }

        let supports_json = body.contains(command_name) && body.contains("--format=json");

        commands.push(CommandRecord {
            repo_id: repo_id.to_string(),
            command_name: command_name.to_string(),
            display_name: command_name.replace('-', " "),
            description: pending_comment.take(),
            invocation: vec!["just".to_string(), command_name.to_string()],
            supports_structured_output: supports_json,
            structured_output_format: if supports_json {
                StructuredOutputFormat::Json
            } else {
                StructuredOutputFormat::Unknown
            },
            tags: Vec::new(),
        });
    }

    Ok(commands)
}

fn is_non_recipe_line(line: &str) -> bool {
    line.starts_with("set ")
        || line.starts_with("import ")
        || line.starts_with("mod ")
        || line.starts_with("alias ")
        || line.starts_with('[')
        || line.contains(":=")
        || line.contains(" = ")
}

fn is_valid_recipe_name(name: &str) -> bool {
    name.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[cfg(test)]
mod tests {
    use super::parse;
    use std::fs;

    #[test]
    fn parses_simple_recipes() {
        let dir =
            std::env::temp_dir().join(format!("youbot-rs-justfile-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("justfile"),
            "# Show tasks\nlist:\n  echo hi\n\nset shell := [\"bash\", \"-lc\"]\nrun-search query:\n  echo ok\n",
        )
        .unwrap();

        let commands = parse("repo", &dir).unwrap();
        let names: Vec<_> = commands
            .into_iter()
            .map(|command| command.command_name)
            .collect();
        assert_eq!(names, vec!["list".to_string(), "run-search".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
