use anyhow::{Context, Result};
use chrono::Utc;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn atomic_write(path: &Path, body: impl AsRef<[u8]>) -> Result<()> {
    let body = body.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let tmp_path = temp_path_for(path);
    {
        let mut file = File::create(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        file.write_all(body)
            .with_context(|| format!("failed to write {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to sync {}", tmp_path.display()))?;
    }

    fs::rename(&tmp_path, path)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    sync_parent_dir(path)?;
    Ok(())
}

pub fn quarantine_corrupt(path: &Path) -> Result<PathBuf> {
    let quarantine_path = corrupt_path_for(path);
    fs::rename(path, &quarantine_path).with_context(|| {
        format!(
            "failed to move corrupt file {} to {}",
            path.display(),
            quarantine_path.display()
        )
    })?;
    Ok(quarantine_path)
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("state"));
    name.push(".tmp");
    path.with_file_name(name)
}

fn corrupt_path_for(path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d%H%M%S");
    let mut name = path
        .file_name()
        .map(OsString::from)
        .unwrap_or_else(|| OsString::from("state"));
    name.push(format!(".corrupt-{timestamp}"));
    path.with_file_name(name)
}

fn sync_parent_dir(path: &Path) -> Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let dir = OpenOptions::new()
        .read(true)
        .open(parent)
        .with_context(|| format!("failed to open {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to sync {}", parent.display()))
}
