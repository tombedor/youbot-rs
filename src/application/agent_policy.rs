use crate::domain::{SessionState, TaskStatus};

pub fn classify_task_title(description: &str) -> String {
    let compact = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return "Untitled task".to_string();
    }

    let cleaned = compact
        .trim_end_matches(['.', '!', '?', ';', ':'])
        .to_string();
    let mut words = cleaned.split_whitespace();
    let title = words.by_ref().take(7).collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        "Untitled task".to_string()
    } else if words.next().is_some() {
        format!("{title}...")
    } else {
        title
    }
}

pub fn prompt_for_completion(transcript: &str) -> Option<String> {
    let lower = transcript.to_ascii_lowercase();
    if lower.contains("waiting for user") || lower.contains("need your input") {
        return Some(
            "Continue autonomously if possible. If you are blocked, state the blocker and the next best action."
                .to_string(),
        );
    }
    None
}

pub fn summarize_transcript(transcript: &str) -> String {
    let mut lines: Vec<&str> = transcript
        .lines()
        .rev()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(4)
        .collect();
    lines.reverse();
    let summary = lines.join(" | ");
    if summary.is_empty() {
        "No transcript captured.".to_string()
    } else {
        summary
    }
}

pub fn infer_task_status(transcript: &str) -> Option<TaskStatus> {
    let lower = transcript.to_ascii_lowercase();
    if lower.contains("wont do") || lower.contains("won't do") {
        Some(TaskStatus::WontDo)
    } else if lower.contains("done")
        || lower.contains("completed")
        || lower.contains("merged")
        || lower.contains("fixed")
    {
        Some(TaskStatus::Complete)
    } else if lower.contains("stuck") || lower.contains("blocked") {
        Some(TaskStatus::InProgress)
    } else {
        None
    }
}

pub fn infer_background_state(transcript: &str) -> SessionState {
    let lower = transcript.to_ascii_lowercase();
    if lower.contains("waiting for user") || lower.contains("need your input") {
        SessionState::WaitingForInput
    } else if lower.contains("done") || lower.contains("completed") || lower.contains("merged") {
        SessionState::Completed
    } else if lower.contains("stuck") || lower.contains("blocked") {
        SessionState::Stuck
    } else {
        SessionState::Active
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_task_title, infer_background_state, infer_task_status, prompt_for_completion,
        summarize_transcript,
    };
    use crate::domain::{SessionState, TaskStatus};

    #[test]
    fn classify_task_title_trims_and_compacts_input() {
        let title = classify_task_title("  Implement the long-running background poller   now.  ");

        assert_eq!(title, "Implement the long-running background poller now");
    }

    #[test]
    fn infer_task_status_marks_completed_work() {
        let status = infer_task_status("Investigated issue\nApplied fix\nCompleted");

        assert_eq!(status, Some(TaskStatus::Complete));
    }

    #[test]
    fn infer_background_state_detects_waiting_for_input() {
        let state = infer_background_state("Need your input before I can continue");

        assert_eq!(state, SessionState::WaitingForInput);
    }

    #[test]
    fn prompt_for_completion_only_for_waiting_transcripts() {
        assert!(prompt_for_completion("waiting for user input").is_some());
        assert!(prompt_for_completion("still coding").is_none());
    }

    #[test]
    fn summarize_transcript_uses_recent_non_empty_lines() {
        let summary = summarize_transcript("one\n\n two \nthree\nfour\nfive");

        assert_eq!(summary, "two | three | four | five");
    }
}
