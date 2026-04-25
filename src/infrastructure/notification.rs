use anyhow::{Result, anyhow};
use std::process::Command;

pub trait NotificationSink: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct SystemNotifier;

impl SystemNotifier {
    fn notify_impl(&self, title: &str, body: &str) -> Result<()> {
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

impl NotificationSink for SystemNotifier {
    fn notify(&self, title: &str, body: &str) -> Result<()> {
        self.notify_impl(title, body)
    }
}
