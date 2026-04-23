use crate::models::{
    CodingAgentActivity, CodingAgentConfig, CodingAgentRunLog, CodingAgentSessionRef, RepoRecord,
};
use crate::persistence;
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

pub struct CodingAgentInvocation {
    pub summary: String,
    pub backend_name: String,
    pub session_id: Option<String>,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_code_change(
    repo: &RepoRecord,
    request: &str,
    config: &CodingAgentConfig,
) -> Result<CodingAgentInvocation> {
    let sessions = persistence::load_sessions()?;
    let preferred_backend = repo
        .preferred_backend
        .as_deref()
        .unwrap_or(&config.default_backend);
    let started_at = Utc::now().to_rfc3339();
    let run_id = Uuid::new_v4().to_string();

    let existing_session = sessions
        .get(&repo.repo_id)
        .filter(|session| session.backend_name == preferred_backend)
        .cloned();

    let request_summary: String = request.chars().take(160).collect();
    let activity = ActivityContext {
        run_id: &run_id,
        repo,
        backend_name: preferred_backend,
        request_summary: &request_summary,
    };
    write_activity(
        &activity,
        existing_session
            .as_ref()
            .map(|session| session.session_id.clone()),
        "started",
        &[format!(
            "Starting {preferred_backend} run for {}",
            repo.name
        )],
    )?;
    persistence::append_activity_event(&serde_json::json!({
        "run_id": run_id,
        "event_kind": "started",
        "repo_id": repo.repo_id,
        "backend_name": preferred_backend,
        "timestamp": Utc::now().to_rfc3339(),
        "content": request,
    }))?;

    let output = match preferred_backend {
        "claude_code" => run_claude(
            repo,
            request,
            existing_session.as_ref(),
            &run_id,
            &request_summary,
        )?,
        _ => run_codex(
            repo,
            request,
            existing_session.as_ref(),
            &run_id,
            &request_summary,
        )?,
    };

    let finished_at = Utc::now().to_rfc3339();
    let detected_session = output.session_id.clone().or_else(|| {
        existing_session
            .as_ref()
            .map(|session| session.session_id.clone())
    });

    if let Some(session_id) = &detected_session {
        persistence::store_session(&CodingAgentSessionRef {
            repo_id: repo.repo_id.clone(),
            backend_name: preferred_backend.to_string(),
            session_kind: "noninteractive".to_string(),
            session_id: session_id.clone(),
            purpose_summary: Some(request.chars().take(120).collect()),
            status: "active".to_string(),
            last_used_at: finished_at.clone(),
        })?;
    }

    write_activity(
        &activity,
        detected_session.clone(),
        if output.exit_code == 0 {
            "completed"
        } else {
            "failed"
        },
        &first_non_empty_lines(
            if output.exit_code == 0 {
                &output.stdout
            } else {
                &output.stderr
            },
            8,
        ),
    )?;
    persistence::append_activity_event(&serde_json::json!({
        "run_id": run_id,
        "event_kind": "finished",
        "repo_id": repo.repo_id,
        "backend_name": preferred_backend,
        "session_id": detected_session,
        "exit_code": output.exit_code,
        "timestamp": Utc::now().to_rfc3339(),
    }))?;

    persistence::append_coding_agent_run(&CodingAgentRunLog {
        repo_id: repo.repo_id.clone(),
        backend_name: preferred_backend.to_string(),
        session_id: output.session_id.clone().or_else(|| {
            existing_session
                .as_ref()
                .map(|session| session.session_id.clone())
        }),
        request_summary: Some(request_summary),
        exit_code: output.exit_code,
        started_at,
        finished_at,
    })?;
    Ok(CodingAgentInvocation {
        summary: summarize_run(
            repo,
            request,
            preferred_backend,
            output.exit_code,
            &output.stdout,
            &output.stderr,
        ),
        backend_name: preferred_backend.to_string(),
        session_id: output.session_id.or_else(|| {
            existing_session
                .as_ref()
                .map(|session| session.session_id.clone())
        }),
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

struct CommandOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
    session_id: Option<String>,
}

enum StreamEvent {
    Stdout(String),
    Stderr(String),
    Closed,
}

struct ActivityContext<'a> {
    run_id: &'a str,
    repo: &'a RepoRecord,
    backend_name: &'a str,
    request_summary: &'a str,
}

fn run_codex(
    repo: &RepoRecord,
    request: &str,
    session: Option<&CodingAgentSessionRef>,
    run_id: &str,
    request_summary: &str,
) -> Result<CommandOutput> {
    let mut command = Command::new("codex");
    command.current_dir(&repo.path);
    command.arg("exec");
    if let Some(session) = session {
        command.arg("resume").arg(&session.session_id);
    }
    command.arg("--json").arg("-C").arg(&repo.path).arg(request);
    run_streaming_command(command, repo, "codex", run_id, request_summary)
}

fn run_claude(
    repo: &RepoRecord,
    request: &str,
    session: Option<&CodingAgentSessionRef>,
    run_id: &str,
    request_summary: &str,
) -> Result<CommandOutput> {
    let mut command = Command::new("claude");
    command
        .current_dir(&repo.path)
        .arg("--print")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--permission-mode")
        .arg("bypassPermissions");
    if let Some(session) = session {
        command.arg("--resume").arg(&session.session_id);
    }
    command.arg(request);
    run_streaming_command(command, repo, "claude_code", run_id, request_summary)
}

fn run_streaming_command(
    mut command: Command,
    repo: &RepoRecord,
    backend_name: &str,
    run_id: &str,
    request_summary: &str,
) -> Result<CommandOutput> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed running {backend_name} in {}", repo.path.display()))?;

    let stdout = child.stdout.take().context("missing child stdout")?;
    let stderr = child.stderr.take().context("missing child stderr")?;
    let (tx, rx) = mpsc::channel();

    spawn_reader(stdout, true, tx.clone());
    spawn_reader(stderr, false, tx);

    let mut stdout_buffer = String::new();
    let mut stderr_buffer = String::new();
    let mut recent_entries = vec![format!("Started {backend_name} in {}", repo.name)];
    let mut open_streams = 2usize;
    let mut session_id = None;
    let activity = ActivityContext {
        run_id,
        repo,
        backend_name,
        request_summary,
    };

    while open_streams > 0 {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(StreamEvent::Stdout(line)) => {
                stdout_buffer.push_str(&line);
                stdout_buffer.push('\n');
                if session_id.is_none() {
                    session_id = detect_session_id(&line);
                }
                push_recent_entry(&mut recent_entries, format!("stdout: {line}"));
                publish_stream_event(
                    &activity,
                    session_id.clone(),
                    "stdout",
                    &line,
                    &recent_entries,
                )?;
            }
            Ok(StreamEvent::Stderr(line)) => {
                stderr_buffer.push_str(&line);
                stderr_buffer.push('\n');
                if session_id.is_none() {
                    session_id = detect_session_id(&line);
                }
                push_recent_entry(&mut recent_entries, format!("stderr: {line}"));
                publish_stream_event(
                    &activity,
                    session_id.clone(),
                    "stderr",
                    &line,
                    &recent_entries,
                )?;
            }
            Ok(StreamEvent::Closed) => open_streams = open_streams.saturating_sub(1),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                write_activity(&activity, session_id.clone(), "running", &recent_entries)?;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let status = child
        .wait()
        .context("failed waiting for coding-agent process")?;
    Ok(CommandOutput {
        exit_code: status.code().unwrap_or(-1),
        stdout: stdout_buffer,
        stderr: stderr_buffer,
        session_id,
    })
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    stream: R,
    is_stdout: bool,
    tx: mpsc::Sender<StreamEvent>,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            let event = if is_stdout {
                StreamEvent::Stdout(line)
            } else {
                StreamEvent::Stderr(line)
            };
            if tx.send(event).is_err() {
                return;
            }
        }
        let _ = tx.send(StreamEvent::Closed);
    });
}

fn publish_stream_event(
    activity: &ActivityContext<'_>,
    session_id: Option<String>,
    event_kind: &str,
    content: &str,
    recent_entries: &[String],
) -> Result<()> {
    write_activity(activity, session_id, "running", recent_entries)?;
    persistence::append_activity_event(&serde_json::json!({
        "run_id": activity.run_id,
        "event_kind": event_kind,
        "repo_id": activity.repo.repo_id,
        "backend_name": activity.backend_name,
        "timestamp": Utc::now().to_rfc3339(),
        "content": content,
    }))
}

fn write_activity(
    activity: &ActivityContext<'_>,
    session_id: Option<String>,
    status: &str,
    recent_entries: &[String],
) -> Result<()> {
    persistence::write_current_activity(&CodingAgentActivity {
        run_id: activity.run_id.to_string(),
        target_repo_id: activity.repo.repo_id.clone(),
        target_kind: "repo".to_string(),
        backend_name: activity.backend_name.to_string(),
        request_summary: activity.request_summary.to_string(),
        session_id,
        status: status.to_string(),
        recent_entries: recent_entries.to_vec(),
    })
}

fn push_recent_entry(entries: &mut Vec<String>, line: String) {
    entries.push(line);
    if entries.len() > 8 {
        let extra = entries.len() - 8;
        entries.drain(0..extra);
    }
}

fn detect_session_id(stream: &str) -> Option<String> {
    for line in stream.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line)
            && let Some(found) = find_session_id(&value)
        {
            return Some(found);
        }
        for token in line.split(|ch: char| ch.is_whitespace() || ch == '"' || ch == ',') {
            if let Ok(uuid) = Uuid::parse_str(token) {
                return Some(uuid.to_string());
            }
        }
    }
    None
}

fn find_session_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in [
                "session_id",
                "sessionId",
                "conversation_id",
                "conversationId",
            ] {
                if let Some(Value::String(session_id)) = map.get(key)
                    && Uuid::parse_str(session_id).is_ok()
                {
                    return Some(session_id.clone());
                }
            }
            map.values().find_map(find_session_id)
        }
        Value::Array(values) => values.iter().find_map(find_session_id),
        _ => None,
    }
}

fn summarize_run(
    repo: &RepoRecord,
    request: &str,
    backend: &str,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) -> String {
    let prefix = format!(
        "Started a {} coding-agent run in {} for: {}",
        backend, repo.name, request
    );
    if exit_code == 0 {
        let lines = first_non_empty_lines(stdout, 10);
        if lines.is_empty() {
            format!("{prefix}\n\nRun completed successfully.")
        } else {
            format!("{prefix}\n\n{}", lines.join("\n"))
        }
    } else {
        let lines = first_non_empty_lines(stderr, 10);
        if lines.is_empty() {
            format!("{prefix}\n\nRun failed with exit code {exit_code}.")
        } else {
            format!(
                "{prefix}\n\nRun failed with exit code {exit_code}.\n{}",
                lines.join("\n")
            )
        }
    }
}

fn first_non_empty_lines(stream: &str, limit: usize) -> Vec<String> {
    stream
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(limit)
        .map(ToString::to_string)
        .collect()
}
