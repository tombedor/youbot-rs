use crate::adapter;
use crate::coding_agent;
use crate::config;
use crate::conversation_store::ConversationStore;
use crate::executor;
use crate::models::{
    AppConfig, CodingAgentActivity, MessageRole, RepoClassification, RepoConfig, RepoOverview,
    RepoRecord, RepoStatus, RouteAction,
};
use crate::openai_chat;
use crate::overview;
use crate::persistence;
use crate::registry;
use crate::router;
use anyhow::Result;
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

pub struct App {
    pub config: AppConfig,
    pub repos: Vec<RepoRecord>,
    pub conversation: ConversationStore,
    pub repo_cursor: usize,
    pub active_repo: Option<usize>,
    pub command_cursor: usize,
    pub focus: Focus,
    pub input: String,
    pub status: String,
    pub active_overview: Option<RepoOverview>,
    pub latest_activity: Option<CodingAgentActivity>,
    pub pending_chat: Option<Receiver<ProcessedMessage>>,
    pub palette_query: String,
    pub palette_cursor: usize,
    pub palette_return_focus: Focus,
    pub should_quit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Repos,
    Input,
    Commands,
    Palette,
}

#[derive(Debug)]
pub struct ProcessedMessage {
    pub conversation_entries: Vec<(MessageRole, String)>,
    pub active_repo: Option<usize>,
    pub status: String,
    pub response_id: Option<String>,
    pub latest_activity: Option<CodingAgentActivity>,
}

#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub title: String,
    pub subtitle: String,
    pub scope: String,
    pub action: PaletteAction,
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    ActivateRepo(usize),
    ClearRepo,
    Reload,
    RunScheduled,
    ReviewUsage,
    RefreshOverview,
    RunCommand {
        repo_index: usize,
        command_name: String,
        arguments: Vec<String>,
    },
}

impl App {
    pub fn load() -> Result<Self> {
        let config = config::load_or_create()?;
        let repos = registry::load(&config)?;
        let conversation = ConversationStore::load_or_create()?;
        Ok(Self {
            config,
            repos,
            conversation,
            repo_cursor: 0,
            active_repo: None,
            command_cursor: 0,
            focus: Focus::Repos,
            input: String::new(),
            status: "Ready. Tab switches focus. Enter selects or runs.".to_string(),
            active_overview: None,
            latest_activity: persistence::load_current_activity().ok().flatten(),
            pending_chat: None,
            palette_query: String::new(),
            palette_cursor: 0,
            palette_return_focus: Focus::Repos,
            should_quit: false,
        })
    }

    pub fn reload(&mut self) -> Result<()> {
        self.config = config::load_or_create()?;
        self.repos = registry::load(&self.config)?;
        if self.repo_cursor >= self.repos.len() && !self.repos.is_empty() {
            self.repo_cursor = self.repos.len() - 1;
        }
        if let Some(active) = self.active_repo
            && active >= self.repos.len()
        {
            self.active_repo = None;
            self.active_overview = None;
        }
        self.command_cursor = 0;
        self.latest_activity = persistence::load_current_activity().ok().flatten();
        self.status = "Reloaded repo config and command inventory.".to_string();
        Ok(())
    }

    pub fn poll_background(&mut self) {
        self.latest_activity = persistence::load_current_activity().ok().flatten();
        let Some(receiver) = self.pending_chat.as_ref() else {
            return;
        };
        match receiver.try_recv() {
            Ok(outcome) => {
                self.pending_chat = None;
                if let Some(repo_index) = outcome.active_repo {
                    self.active_repo = Some(repo_index);
                    self.repo_cursor = repo_index;
                }
                if let Some(response_id) = outcome.response_id {
                    let _ = self.conversation.set_last_response_id(Some(response_id));
                }
                for (role, body) in outcome.conversation_entries {
                    let _ = self.conversation.append(role, body);
                }
                self.latest_activity = outcome.latest_activity;
                self.status = outcome.status;
                let _ = self.refresh_active_overview();
            }
            Err(TryRecvError::Disconnected) => {
                self.pending_chat = None;
                self.status = "Background chat task terminated unexpectedly.".to_string();
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    pub fn is_processing(&self) -> bool {
        self.pending_chat.is_some()
    }

    pub fn active_repo(&self) -> Option<&RepoRecord> {
        self.active_repo.and_then(|index| self.repos.get(index))
    }

    pub fn active_commands_len(&self) -> usize {
        self.active_overview
            .as_ref()
            .map(|overview| overview.quick_actions.len())
            .unwrap_or(0)
    }

    pub fn select_next(&mut self) {
        match self.focus {
            Focus::Repos => {
                if !self.repos.is_empty() {
                    self.repo_cursor = (self.repo_cursor + 1) % self.repos.len();
                }
            }
            Focus::Commands => {
                let len = self.active_commands_len();
                if len > 0 {
                    self.command_cursor = (self.command_cursor + 1) % len;
                }
            }
            Focus::Input => {}
            Focus::Palette => {
                let len = self.filtered_palette_entries().len();
                if len > 0 {
                    self.palette_cursor = (self.palette_cursor + 1) % len;
                }
            }
        }
    }

    pub fn select_previous(&mut self) {
        match self.focus {
            Focus::Repos => {
                if !self.repos.is_empty() {
                    self.repo_cursor = if self.repo_cursor == 0 {
                        self.repos.len() - 1
                    } else {
                        self.repo_cursor - 1
                    };
                }
            }
            Focus::Commands => {
                let len = self.active_commands_len();
                if len > 0 {
                    self.command_cursor = if self.command_cursor == 0 {
                        len - 1
                    } else {
                        self.command_cursor - 1
                    };
                }
            }
            Focus::Input => {}
            Focus::Palette => {
                let len = self.filtered_palette_entries().len();
                if len > 0 {
                    self.palette_cursor = if self.palette_cursor == 0 {
                        len - 1
                    } else {
                        self.palette_cursor - 1
                    };
                }
            }
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match (self.focus, self.active_repo.is_some()) {
            (Focus::Repos, _) => Focus::Input,
            (Focus::Input, true) => Focus::Commands,
            (Focus::Input, false) => Focus::Repos,
            (Focus::Commands, _) => Focus::Repos,
            (Focus::Palette, _) => self.palette_return_focus,
        };
    }

    pub fn activate_selected_repo(&mut self) {
        if self.repos.is_empty() {
            return;
        }
        self.active_repo = Some(self.repo_cursor);
        self.command_cursor = 0;
        if let Some(repo) = self.repos.get(self.repo_cursor) {
            self.config.ui.last_active_repo_id = Some(repo.repo_id.clone());
            let _ = config::save(&self.config);
        }
        let repo = &self.repos[self.repo_cursor];
        self.status = format!(
            "Active repo: {} ({}, {} commands)",
            repo.name,
            repo.status.label(),
            repo.commands.len()
        );
        if let Err(error) = self.refresh_active_overview() {
            self.status = format!("Active repo set, but overview refresh failed: {error:#}");
        }
    }

    pub fn clear_active_repo(&mut self) {
        self.active_repo = None;
        self.active_overview = None;
        self.command_cursor = 0;
        self.focus = Focus::Repos;
        self.status = "Returned to global chat with no active repo.".to_string();
    }

    pub fn open_palette(&mut self) {
        self.palette_return_focus = self.focus;
        self.focus = Focus::Palette;
        self.palette_query.clear();
        self.palette_cursor = 0;
        self.status = "Command palette opened.".to_string();
    }

    pub fn close_palette(&mut self) {
        self.focus = self.palette_return_focus;
        self.palette_query.clear();
        self.palette_cursor = 0;
    }

    pub fn submit_input(&mut self) -> Result<()> {
        if self.pending_chat.is_some() {
            self.status = "A chat request is already in progress.".to_string();
            return Ok(());
        }

        let message = self.input.trim().to_string();
        if message.is_empty() {
            return Ok(());
        }

        self.conversation.append(MessageRole::User, &message)?;

        if self.handle_slash_command(&message)? {
            self.input.clear();
            return Ok(());
        }

        let repos = self.repos.clone();
        let config = self.config.clone();
        let active_repo = self.active_repo;
        let previous_response_id = self.conversation.record().last_response_id.clone();
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let outcome = process_message(
                message,
                repos,
                config,
                active_repo,
                previous_response_id.as_deref(),
            );
            let _ = tx.send(outcome);
        });

        self.pending_chat = Some(rx);
        self.status = "Processing chat request...".to_string();
        self.input.clear();
        Ok(())
    }

    pub fn run_selected_command(&mut self) -> Result<()> {
        let Some(active_index) = self.active_repo else {
            self.status = "No active repo selected.".to_string();
            return Ok(());
        };
        let Some(action) = self
            .active_overview
            .as_ref()
            .and_then(|overview| overview.quick_actions.get(self.command_cursor))
            .cloned()
        else {
            self.status = "No quick actions are available for the active repo.".to_string();
            return Ok(());
        };

        self.run_command(active_index, &action.command_name, &action.arguments)
    }

    pub fn refresh_active_overview(&mut self) -> Result<()> {
        let Some(active_index) = self.active_repo else {
            self.active_overview = None;
            return Ok(());
        };
        let repo = &self.repos[active_index];
        self.active_overview = Some(overview::build(repo)?);
        Ok(())
    }

    fn run_command(
        &mut self,
        repo_index: usize,
        command_name: &str,
        arguments: &[String],
    ) -> Result<()> {
        let repo = &self.repos[repo_index];
        if repo.status != RepoStatus::Ready {
            self.status = format!("Repo {} is not ready.", repo.name);
            return Ok(());
        }

        self.status = format!("Running `just {command_name}` in {}...", repo.name);
        let result = executor::run(repo, command_name, arguments)?;

        self.conversation.append(
            MessageRole::Tool,
            format!(
                "Executed in {}: `{}` (exit {})",
                repo.name,
                result.invocation.join(" "),
                result.exit_code
            ),
        )?;
        self.conversation.append(
            MessageRole::Assistant,
            overview::summarize_execution(repo, &result),
        )?;
        self.status = format!(
            "Completed `just {command_name}` with exit code {}.",
            result.exit_code
        );

        self.active_repo = Some(repo_index);
        self.repo_cursor = repo_index;
        let _ = self.refresh_active_overview();
        Ok(())
    }

    fn run_review_usage_flow(&mut self) -> Result<()> {
        let bundle = persistence::create_review_bundle(self.conversation.record())?;
        let youbot_repo_index = self
            .repos
            .iter()
            .position(|repo| repo.repo_id == "youbot" || repo.name == "youbot-rs");

        let mut response = format!(
            "Generated a usage review bundle for this installation.\n\nBundle: {}\nWindow: {} messages, recent command runs, coding-agent runs, and activity events included.",
            bundle.display(),
            self.conversation.record().messages.len()
        );

        if let Some(repo_index) = youbot_repo_index {
            let repo = &self.repos[repo_index];
            let request = format!(
                "Review the usage bundle at {}. Analyze recent conversations, command runs, coding-agent activity, adapters, and UX behavior. Suggest concrete product, routing, adapter, and UI improvements for youbot-rs.",
                bundle.display()
            );
            let result = coding_agent::run_code_change(repo, &request, &self.config.coding_agent)?;
            self.active_repo = Some(repo_index);
            self.repo_cursor = repo_index;
            response.push_str("\n\n");
            response.push_str(&result.summary);
            self.latest_activity = persistence::load_current_activity().ok().flatten();
            let _ = self.refresh_active_overview();
            self.status = "Generated review bundle and launched coding-agent review.".to_string();
        } else {
            self.status = "Generated review bundle.".to_string();
        }

        self.conversation
            .append(MessageRole::Assistant, response.clone())?;
        Ok(())
    }

    pub fn filtered_palette_entries(&self) -> Vec<PaletteEntry> {
        let mut entries = vec![
            PaletteEntry {
                title: "Reload registry".to_string(),
                subtitle: "Refresh repo config, command discovery, and adapters".to_string(),
                scope: "global".to_string(),
                action: PaletteAction::Reload,
            },
            PaletteEntry {
                title: "Run scheduled jobs".to_string(),
                subtitle: "Execute configured scheduler jobs now".to_string(),
                scope: "global".to_string(),
                action: PaletteAction::RunScheduled,
            },
            PaletteEntry {
                title: "Review usage".to_string(),
                subtitle: "Build a review bundle and send it to the coding agent".to_string(),
                scope: "global".to_string(),
                action: PaletteAction::ReviewUsage,
            },
        ];

        if self.active_repo.is_some() {
            entries.push(PaletteEntry {
                title: "Clear active repo".to_string(),
                subtitle: "Return to global chat without repo focus".to_string(),
                scope: "global".to_string(),
                action: PaletteAction::ClearRepo,
            });
            entries.push(PaletteEntry {
                title: "Refresh active overview".to_string(),
                subtitle: "Reload adapter-backed overview data".to_string(),
                scope: "global".to_string(),
                action: PaletteAction::RefreshOverview,
            });
        }

        entries.extend(
            self.repos
                .iter()
                .enumerate()
                .map(|(index, repo)| PaletteEntry {
                    title: format!("Switch to {}", repo.name),
                    subtitle: format!("Focus {}", repo.path.display()),
                    scope: "global".to_string(),
                    action: PaletteAction::ActivateRepo(index),
                }),
        );

        if let Some(active_index) = self.active_repo
            && let Some(repo) = self.repos.get(active_index)
        {
            if let Some(overview) = &self.active_overview {
                entries.extend(overview.quick_actions.iter().map(|action| PaletteEntry {
                    title: action.title.clone(),
                    subtitle: format!("Run `just {}`", action.command_name),
                    scope: repo.name.clone(),
                    action: PaletteAction::RunCommand {
                        repo_index: active_index,
                        command_name: action.command_name.clone(),
                        arguments: action.arguments.clone(),
                    },
                }));
            }

            entries.extend(repo.commands.iter().map(|command| {
                PaletteEntry {
                    title: command.display_name.clone(),
                    subtitle: command
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("Run `just {}`", command.command_name)),
                    scope: repo.name.clone(),
                    action: PaletteAction::RunCommand {
                        repo_index: active_index,
                        command_name: command.command_name.clone(),
                        arguments: Vec::new(),
                    },
                }
            }));
        }

        let query = self.palette_query.trim().to_lowercase();
        let filtered = if query.is_empty() {
            entries
        } else {
            entries
                .into_iter()
                .filter(|entry| {
                    entry.title.to_lowercase().contains(&query)
                        || entry.subtitle.to_lowercase().contains(&query)
                        || entry.scope.to_lowercase().contains(&query)
                })
                .collect()
        };

        dedupe_palette_entries(filtered)
    }

    pub fn run_selected_palette_entry(&mut self) -> Result<()> {
        let entries = self.filtered_palette_entries();
        let Some(entry) = entries.get(self.palette_cursor.min(entries.len().saturating_sub(1)))
        else {
            self.status = "No palette actions match the current filter.".to_string();
            return Ok(());
        };

        match &entry.action {
            PaletteAction::ActivateRepo(index) => {
                self.repo_cursor = *index;
                self.activate_selected_repo();
            }
            PaletteAction::ClearRepo => self.clear_active_repo(),
            PaletteAction::Reload => {
                self.reload()?;
            }
            PaletteAction::RunScheduled => {
                self.run_scheduled_jobs()?;
            }
            PaletteAction::ReviewUsage => {
                self.run_review_usage_flow()?;
            }
            PaletteAction::RefreshOverview => {
                self.refresh_active_overview()?;
                self.status = "Refreshed the active repo overview.".to_string();
            }
            PaletteAction::RunCommand {
                repo_index,
                command_name,
                arguments,
            } => {
                self.run_command(*repo_index, command_name, arguments)?;
            }
        }

        self.close_palette();
        Ok(())
    }

    fn handle_slash_command(&mut self, message: &str) -> Result<bool> {
        if message == "/review-usage" {
            self.run_review_usage_flow()?;
            return Ok(true);
        }

        if let Some(rest) = message.strip_prefix("/add-repo ") {
            let path = PathBuf::from(rest.trim());
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("repo")
                .to_string();
            self.add_repo(&path, &name, RepoClassification::Integrated)?;
            return Ok(true);
        }

        if let Some(rest) = message.strip_prefix("/init-managed ") {
            let path = PathBuf::from(rest.trim());
            self.init_managed_repo(&path)?;
            return Ok(true);
        }

        if message == "/run-scheduled" {
            self.run_scheduled_jobs()?;
            return Ok(true);
        }

        Ok(false)
    }

    fn add_repo(
        &mut self,
        path: &Path,
        name: &str,
        classification: RepoClassification,
    ) -> Result<()> {
        let repo_id = name.replace(' ', "_");
        let exists = self.config.repos.iter().any(|repo| repo.repo_id == repo_id);
        if !exists {
            self.config.repos.push(RepoConfig {
                repo_id: repo_id.clone(),
                name: name.to_string(),
                path: path.to_path_buf(),
                classification,
            });
            config::save(&self.config)?;
            self.reload()?;
        }
        self.conversation.append(
            MessageRole::Assistant,
            format!("Registered repo `{repo_id}` at {}.", path.display()),
        )?;
        self.status = format!("Registered repo `{repo_id}`.");
        Ok(())
    }

    fn init_managed_repo(&mut self, path: &Path) -> Result<()> {
        fs::create_dir_all(path.join("src"))?;
        let repo_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("managed_repo");
        fs::write(
            path.join("PRD.md"),
            format!("# {repo_name}\n\nDescribe the repo.\n"),
        )?;
        fs::write(
            path.join("AGENTS.md"),
            "# AGENTS\n\nDocument architecture, constraints, and coding patterns.\n",
        )?;
        fs::write(
            path.join("CAPTAINS_LOG.md"),
            format!(
                "## {}\n- Initialized managed repo scaffold.\n",
                Utc::now().date_naive()
            ),
        )?;
        fs::write(
            path.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n",
                repo_name.replace('-', "_")
            ),
        )?;
        fs::write(
            path.join("justfile"),
            "install:\n    cargo fetch\n\nformat:\n    cargo fmt\n\nlint:\n    cargo clippy --all-targets --all-features -- -D warnings\n\ntest:\n    cargo test\n",
        )?;
        fs::write(
            path.join("src/main.rs"),
            "fn main() {\n    println!(\"managed repo scaffold\");\n}\n",
        )?;
        self.add_repo(path, repo_name, RepoClassification::Managed)?;
        self.status = format!("Initialized managed repo scaffold at {}.", path.display());
        Ok(())
    }

    fn run_scheduled_jobs(&mut self) -> Result<()> {
        let mut summaries = Vec::new();
        for job in &self.config.scheduler.jobs {
            if let Some(repo) = self.repos.iter().find(|repo| repo.repo_id == job.repo_id) {
                let result = executor::run(repo, &job.command_name, &[])?;
                persistence::append_scheduler_run(&serde_json::json!({
                    "job_id": job.job_id,
                    "repo_id": job.repo_id,
                    "command_name": job.command_name,
                    "started_at": result.started_at,
                    "finished_at": result.finished_at,
                    "exit_code": result.exit_code,
                }))?;
                summaries.push(format!(
                    "{} -> {} (exit {})",
                    job.job_id, job.command_name, result.exit_code
                ));
            }
        }
        let message = if summaries.is_empty() {
            "No scheduled jobs configured.".to_string()
        } else {
            format!("Ran scheduled jobs:\n{}", summaries.join("\n"))
        };
        self.conversation
            .append(MessageRole::Assistant, message.clone())?;
        self.status = message;
        Ok(())
    }
}

#[cfg(test)]
impl App {
    pub fn for_test(
        repos: Vec<RepoRecord>,
        conversation: ConversationStore,
        active_repo: Option<usize>,
        active_overview: Option<RepoOverview>,
    ) -> Self {
        Self {
            config: AppConfig::default(),
            repos,
            conversation,
            repo_cursor: active_repo.unwrap_or(0),
            active_repo,
            command_cursor: 0,
            focus: Focus::Repos,
            input: String::new(),
            status: "Ready. Tab switches focus. Enter selects or runs.".to_string(),
            active_overview,
            latest_activity: None,
            pending_chat: None,
            palette_query: String::new(),
            palette_cursor: 0,
            palette_return_focus: Focus::Repos,
            should_quit: false,
        }
    }
}

fn process_message(
    message: String,
    repos: Vec<RepoRecord>,
    config: AppConfig,
    active_repo: Option<usize>,
    previous_response_id: Option<&str>,
) -> ProcessedMessage {
    if std::env::var("OPENAI_API_KEY").is_ok() {
        match openai_chat::respond(
            &message,
            &repos,
            active_repo,
            previous_response_id,
            &config.coding_agent,
        ) {
            Ok(response) => {
                return ProcessedMessage {
                    conversation_entries: vec![(MessageRole::Assistant, response.assistant_text)],
                    active_repo,
                    status: "Handled by OpenAI tool-calling orchestration.".to_string(),
                    response_id: response.response_id,
                    latest_activity: persistence::load_current_activity().ok().flatten(),
                };
            }
            Err(error) => {
                let mut outcome = process_message_local(&message, &repos, &config, active_repo);
                outcome.conversation_entries.insert(
                    0,
                    (
                        MessageRole::Tool,
                        format!("OpenAI orchestration failed, falling back locally: {error:#}"),
                    ),
                );
                return outcome;
            }
        }
    }

    process_message_local(&message, &repos, &config, active_repo)
}

fn process_message_local(
    message: &str,
    repos: &[RepoRecord],
    config: &AppConfig,
    active_repo: Option<usize>,
) -> ProcessedMessage {
    if let Some(decision) = router::route_message(message, repos, active_repo) {
        let repo = &repos[decision.repo_index];
        match decision.action {
            RouteAction::Command => {
                match executor::run(repo, &decision.command_name, &decision.args) {
                    Ok(result) => ProcessedMessage {
                        conversation_entries: vec![
                            (
                                MessageRole::Tool,
                                format!(
                                    "Router selected {} -> `just {} {}` ({})",
                                    repo.name,
                                    decision.command_name,
                                    decision.args.join(" "),
                                    decision.reasoning
                                ),
                            ),
                            (
                                MessageRole::Assistant,
                                overview::summarize_execution(repo, &result),
                            ),
                        ],
                        active_repo: Some(decision.repo_index),
                        status: format!(
                            "Routed to {} and ran `just {}`.",
                            repo.name, decision.command_name
                        ),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                    Err(error) => ProcessedMessage {
                        conversation_entries: vec![(
                            MessageRole::Assistant,
                            format!("Command execution failed in {}: {error:#}", repo.name),
                        )],
                        active_repo: Some(decision.repo_index),
                        status: format!("Command execution failed in {}.", repo.name),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                }
            }
            RouteAction::CodeChange => {
                let request = decision.prompt.as_deref().unwrap_or(message);
                match coding_agent::run_code_change(repo, request, &config.coding_agent) {
                    Ok(result) => ProcessedMessage {
                        conversation_entries: vec![
                            (
                                MessageRole::Tool,
                                format!(
                                    "Router selected {} -> coding-agent backend ({})",
                                    repo.name, decision.reasoning
                                ),
                            ),
                            (MessageRole::Assistant, result.summary),
                        ],
                        active_repo: Some(decision.repo_index),
                        status: format!(
                            "Ran {} coding-agent backend for {}.",
                            result.backend_name, repo.name
                        ),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                    Err(error) => ProcessedMessage {
                        conversation_entries: vec![(
                            MessageRole::Assistant,
                            format!("Coding-agent launch failed in {}: {error:#}", repo.name),
                        )],
                        active_repo: Some(decision.repo_index),
                        status: format!("Coding-agent launch failed in {}.", repo.name),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                }
            }
            RouteAction::AdapterChange => {
                let request = decision.prompt.as_deref().unwrap_or(message);
                match adapter::apply_change(repo, request) {
                    Ok(summary) => ProcessedMessage {
                        conversation_entries: vec![
                            (
                                MessageRole::Tool,
                                format!(
                                    "Router selected {} -> adapter change ({})",
                                    repo.name, decision.reasoning
                                ),
                            ),
                            (MessageRole::Assistant, summary),
                        ],
                        active_repo: Some(decision.repo_index),
                        status: format!("Updated the {} adapter.", repo.name),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                    Err(error) => ProcessedMessage {
                        conversation_entries: vec![(
                            MessageRole::Assistant,
                            format!("Adapter update failed for {}: {error:#}", repo.name),
                        )],
                        active_repo: Some(decision.repo_index),
                        status: format!("Adapter update failed for {}.", repo.name),
                        response_id: None,
                        latest_activity: persistence::load_current_activity().ok().flatten(),
                    },
                }
            }
            RouteAction::Clarify => ProcessedMessage {
                conversation_entries: vec![(
                    MessageRole::Assistant,
                    format!(
                        "I need clarification before acting in {}. Mention the repo explicitly or tell me the command or change you want.",
                        repo.name
                    ),
                )],
                active_repo: Some(decision.repo_index),
                status: "Waiting for clarification.".to_string(),
                response_id: None,
                latest_activity: persistence::load_current_activity().ok().flatten(),
            },
        }
    } else {
        ProcessedMessage {
            conversation_entries: vec![(
                MessageRole::Assistant,
                "I couldn't confidently map that to a repo command yet. Activate a repo or use an explicit term like pipeline, tasks, research program, findings, or datasets.".to_string(),
            )],
            active_repo,
            status: "No route matched.".to_string(),
            response_id: None,
            latest_activity: persistence::load_current_activity().ok().flatten(),
        }
    }
}

fn dedupe_palette_entries(entries: Vec<PaletteEntry>) -> Vec<PaletteEntry> {
    let mut seen = std::collections::BTreeSet::new();
    let mut deduped = Vec::new();
    for entry in entries {
        let key = format!("{}|{}|{}", entry.scope, entry.title, entry.subtitle);
        if seen.insert(key) {
            deduped.push(entry);
        }
    }
    deduped
}
