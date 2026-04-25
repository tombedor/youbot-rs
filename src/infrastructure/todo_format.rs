use crate::domain::TaskRecord;
use anyhow::{Context, Result, anyhow};

const TODO_HEADER: &str = "# TODO\n\n";
const TODO_MARKER_START: &str = "<!-- youbot:tasks ";
const TODO_MARKER_END: &str = " -->";

pub fn render_todo_markdown(tasks: &[TaskRecord]) -> String {
    let json = serde_json::to_string_pretty(tasks).expect("task serialization should not fail");
    let mut body = String::from(TODO_HEADER);
    body.push_str(TODO_MARKER_START);
    body.push_str(&json);
    body.push_str(TODO_MARKER_END);
    body.push_str("\n\n");

    if tasks.is_empty() {
        body.push_str("_No tasks yet._\n");
        return body;
    }

    for task in tasks {
        body.push_str(&format!("## {} [{}]\n", task.title, task.status.label()));
        body.push_str(&format!("- id: `{}`\n", task.id));
        body.push_str(&format!("- description: {}\n", task.description));
        if task.sessions.is_empty() {
            body.push_str("- sessions: none\n\n");
            continue;
        }
        for session in &task.sessions {
            body.push_str(&format!(
                "- {} {} session: `{}` ({})\n",
                session.product.label(),
                session.session_kind.label(),
                session.session_id,
                session.state.label()
            ));
            if let Some(branch) = &session.branch_name {
                body.push_str(&format!("  branch: `{branch}`\n"));
            }
            if let Some(summary) = &session.last_summary {
                body.push_str(&format!("  last summary: {}\n", summary.summary));
            }
        }
        body.push('\n');
    }

    body
}

pub fn parse_todo_markdown(body: &str) -> Result<Vec<TaskRecord>> {
    let Some(start) = body.find(TODO_MARKER_START) else {
        return Ok(Vec::new());
    };
    let json_start = start + TODO_MARKER_START.len();
    let remaining = &body[json_start..];
    let Some(end) = remaining.find(TODO_MARKER_END) else {
        return Err(anyhow!("missing TODO metadata terminator"));
    };
    let json = &remaining[..end];
    let tasks = serde_json::from_str(json).context("failed to parse TODO metadata")?;
    Ok(tasks)
}
