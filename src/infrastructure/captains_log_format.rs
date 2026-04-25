use crate::domain::CaptainLogEntry;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

pub fn render_captains_log(entries: &[CaptainLogEntry]) -> String {
    #[derive(Serialize)]
    struct Wrapper<'a> {
        entries: &'a [CaptainLogEntry],
    }

    let mut body = String::from("# CAPTAINS LOG\n\n");
    body.push_str("<!-- youbot:captains_log ");
    body.push_str(
        &serde_json::to_string_pretty(&Wrapper { entries })
            .expect("captains log serialization should not fail"),
    );
    body.push_str(" -->\n\n");
    for entry in entries.iter().rev() {
        body.push_str(&format!(
            "## {} | {} | {}\n{}\n\n",
            entry.timestamp.to_rfc3339(),
            entry.task_title,
            entry.product.label(),
            entry.summary
        ));
    }
    body
}

pub fn parse_captains_log(body: &str) -> Result<Vec<CaptainLogEntry>> {
    #[derive(Deserialize)]
    struct Wrapper {
        entries: Vec<CaptainLogEntry>,
    }

    let marker = "<!-- youbot:captains_log ";
    let Some(start) = body.find(marker) else {
        return Ok(Vec::new());
    };
    let json_start = start + marker.len();
    let remaining = &body[json_start..];
    let Some(end) = remaining.find(" -->") else {
        return Err(anyhow!("missing captains log metadata terminator"));
    };
    let wrapper: Wrapper =
        serde_json::from_str(&remaining[..end]).context("failed to parse captains log metadata")?;
    Ok(wrapper.entries)
}
