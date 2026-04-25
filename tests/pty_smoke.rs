use std::fs;
use std::io::Write;
use std::io::{ErrorKind, Result as IoResult};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn pty_smoke_starts_and_quits_cleanly() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();

    let transcript = run_in_pty_steps(&home, &[(b"q".as_slice(), 0)]);

    let config_path = home.join(".youbot").join("config.json");
    assert!(
        config_path.exists(),
        "expected config at {}",
        config_path.display()
    );
    assert!(
        transcript.contains("\u{1b}[?1049h"),
        "transcript was:\n{transcript}"
    );
    assert!(
        transcript.contains("\u{1b}[?1049l"),
        "transcript was:\n{transcript}"
    );
}

#[test]
fn pty_smoke_starts_cleanly_with_existing_state() {
    let temp = tempdir().unwrap();
    let home = temp.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let state_root = home.join(".youbot");
    let managed_root = state_root.join("managed");
    let repo = temp.path().join("repo");
    fs::create_dir_all(&managed_root).unwrap();
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(state_root.join("projects")).unwrap();

    fs::write(
        state_root.join("config.json"),
        format!(
            concat!(
                "{{\n",
                "  \"state_root\": \"{}\",\n",
                "  \"managed_repo_root\": \"{}\",\n",
                "  \"tmux_socket_name\": \"youbot-test\",\n",
                "  \"monitor_silence_seconds\": 120\n",
                "}}\n"
            ),
            escape_json_path(&state_root),
            escape_json_path(&managed_root),
        ),
    )
    .unwrap();
    fs::write(
        state_root.join("projects.json"),
        format!(
            concat!(
                "[{{\n",
                "  \"id\": \"project-1\",\n",
                "  \"name\": \"repo\",\n",
                "  \"path\": \"{}\",\n",
                "  \"created_at\": \"2026-01-01T00:00:00Z\",\n",
                "  \"config\": {{ \"auto_merge\": false }}\n",
                "}}]\n"
            ),
            escape_json_path(&repo),
        ),
    )
    .unwrap();

    let transcript = run_in_pty_steps(&home, &[(b"q".as_slice(), 0)]);
    let projects = fs::read_to_string(state_root.join("projects.json")).unwrap();

    assert!(
        transcript.contains("\u{1b}[?1049h"),
        "transcript was:\n{transcript}"
    );
    assert!(
        projects.contains(&repo.display().to_string()),
        "projects.json was:\n{projects}"
    );
}

fn run_in_pty_steps(home: &Path, steps: &[(&[u8], u64)]) -> String {
    let transcript_path = home.join("typescript");
    let binary = env!("CARGO_BIN_EXE_youbot-rs");

    let mut child = Command::new("/usr/bin/script")
        .args(["-q", transcript_path.to_str().unwrap(), binary])
        .env("HOME", home)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    child.stdin.as_mut().unwrap().flush().unwrap();
    for (bytes, delay_ms) in steps {
        if let Err(error) = write_step(child.stdin.as_mut().unwrap(), bytes) {
            if error.kind() != ErrorKind::BrokenPipe {
                panic!("failed to write PTY input: {error}");
            }
            break;
        }
        if *delay_ms > 0 {
            thread::sleep(Duration::from_millis(*delay_ms));
        }
    }

    let status = child.wait().unwrap();
    assert!(status.success(), "pty run failed with status {status}");

    fs::read_to_string(transcript_path).unwrap()
}

fn write_step(stdin: &mut std::process::ChildStdin, bytes: &[u8]) -> IoResult<()> {
    stdin.write_all(bytes)?;
    stdin.flush()
}

fn escape_json_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}
