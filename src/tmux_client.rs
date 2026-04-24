use anyhow::{Context, Result, anyhow};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TmuxClient {
    socket_name: String,
}

impl TmuxClient {
    pub fn new(socket_name: impl Into<String>) -> Self {
        Self {
            socket_name: socket_name.into(),
        }
    }

    pub fn session_exists(&self, session_name: &str) -> bool {
        Command::new("tmux")
            .args(["-L", &self.socket_name, "has-session", "-t", session_name])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    pub fn create_session(
        &self,
        session_name: &str,
        cwd: &Path,
        command: &str,
        detached: bool,
    ) -> Result<()> {
        let mut args = vec![
            "-L".to_string(),
            self.socket_name.clone(),
            "new-session".to_string(),
            "-s".to_string(),
            session_name.to_string(),
            "-c".to_string(),
            cwd.display().to_string(),
        ];
        if detached {
            args.push("-d".to_string());
        }
        args.push(command.to_string());

        let status = Command::new("tmux")
            .args(&args)
            .status()
            .with_context(|| format!("failed to create tmux session {session_name}"))?;
        if !status.success() {
            return Err(anyhow!("tmux new-session failed for {session_name}"));
        }
        Ok(())
    }

    pub fn attach(&self, session_name: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args(["-L", &self.socket_name, "attach-session", "-t", session_name])
            .status()
            .with_context(|| format!("failed to attach to tmux session {session_name}"))?;
        if !status.success() {
            return Err(anyhow!("tmux attach failed for {session_name}"));
        }
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let output = Command::new("tmux")
            .args(["-L", &self.socket_name, "list-sessions", "-F", "#{session_name}"])
            .output()
            .with_context(|| "failed to list tmux sessions")?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        let sessions = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(sessions)
    }

    pub fn capture_pane(&self, session_name: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args([
                "-L",
                &self.socket_name,
                "capture-pane",
                "-pt",
                session_name,
                "-S",
                "-200",
            ])
            .output()
            .with_context(|| format!("failed to capture tmux pane for {session_name}"))?;
        if !output.status.success() {
            return Err(anyhow!("tmux capture-pane failed for {session_name}"));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn send_keys(&self, session_name: &str, input: &str) -> Result<()> {
        let status = Command::new("tmux")
            .args([
                "-L",
                &self.socket_name,
                "send-keys",
                "-t",
                session_name,
                input,
                "Enter",
            ])
            .status()
            .with_context(|| format!("failed to send keys to {session_name}"))?;
        if !status.success() {
            return Err(anyhow!("tmux send-keys failed for {session_name}"));
        }
        Ok(())
    }

    pub fn enable_monitor_silence(&self, session_name: &str, seconds: u64) -> Result<()> {
        let pane_target = format!("{session_name}:0.0");
        let status = Command::new("tmux")
            .args([
                "-L",
                &self.socket_name,
                "set-option",
                "-pt",
                &pane_target,
                "monitor-silence",
                &seconds.to_string(),
            ])
            .status()
            .with_context(|| format!("failed to configure monitor-silence for {session_name}"))?;
        if !status.success() {
            return Err(anyhow!("tmux set-option failed for {session_name}"));
        }
        Ok(())
    }
}
