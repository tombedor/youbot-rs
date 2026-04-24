use anyhow::{Result, anyhow};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct Notifier;

impl Notifier {
    pub fn notify(&self, title: &str, body: &str) -> Result<()> {
        if cfg!(target_os = "macos") {
            let script = format!("display notification {:?} with title {:?}", body, title);
            let status = Command::new("osascript").args(["-e", &script]).status()?;
            if status.success() {
                return Ok(());
            }
        }

        if cfg!(target_os = "linux") {
            let status = Command::new("notify-send").args([title, body]).status()?;
            if status.success() {
                return Ok(());
            }
        }

        Err(anyhow!("no supported notification backend available"))
    }
}
