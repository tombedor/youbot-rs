# Modules

## Architecture

The codebase is split into four layers:

- `src/domain/`
  Pure data types and invariants.
- `src/application/`
  Use-case orchestration and agent/session policy.
- `src/infrastructure/`
  Filesystem, tmux, notification, git, and repo provisioning adapters.
- `src/ui/`
  Feature-first TUI modules, each with a `handler.rs` and `view.rs`.

`src/app.rs` is the top-level coordinator. It is intentionally thin:

- `AppState` in [src/ui/state.rs](/Users/tombedor/development/youbot-rs/src/ui/state.rs:1)
  Holds route, selections, drafts, status text, and cached project/task/session lists.
- `AppServices` in [src/application/context.rs](/Users/tombedor/development/youbot-rs/src/application/context.rs:1)
  Holds the long-lived service graph and infrastructure dependencies.

## Domain

### Config
- [src/domain/config.rs](/Users/tombedor/development/youbot-rs/src/domain/config.rs:1)
- `AppConfig`
- `ProjectConfig`

### Project
- [src/domain/project.rs](/Users/tombedor/development/youbot-rs/src/domain/project.rs:1)
- `ProjectRecord`

### Task
- [src/domain/task.rs](/Users/tombedor/development/youbot-rs/src/domain/task.rs:1)
- `TaskRecord`
- `TaskStatus`
- `CaptainLogEntry`

### Session
- [src/domain/session.rs](/Users/tombedor/development/youbot-rs/src/domain/session.rs:1)
- `CodingAgentProduct`
- `SessionKind`
- `SessionState`
- `SessionSummary`
- `AgentSessionRef`
- `SessionRecord`

## Application

### AppServices
- [src/application/context.rs](/Users/tombedor/development/youbot-rs/src/application/context.rs:1)
- Builds the service graph used by the TUI.

### ProjectService
- [src/application/project_service.rs](/Users/tombedor/development/youbot-rs/src/application/project_service.rs:1)
- Project-focused application commands.
- Coordinates project registry writes with state-history snapshots.

### TaskService
- [src/application/task_service.rs](/Users/tombedor/development/youbot-rs/src/application/task_service.rs:1)
- Task-focused application commands.
- Owns task creation and status-cycling workflows above raw task storage.

### Agent Policy
- [src/application/agent_policy.rs](/Users/tombedor/development/youbot-rs/src/application/agent_policy.rs:1)
- Pure logic for:
  - task-title classification
  - transcript summarization
  - status inference
  - waiting-for-input prompting

### SessionReviewService
- [src/application/session_review_service.rs](/Users/tombedor/development/youbot-rs/src/application/session_review_service.rs:1)
- Applies transcript policy to stored task state and captain’s log updates.

### SessionService
- [src/application/session_service.rs](/Users/tombedor/development/youbot-rs/src/application/session_service.rs:1)
- Orchestrates live/background session lifecycle:
  - tmux session creation
  - background polling
  - post-attach finalization
  - notification dispatch
  - persisted session state

## Infrastructure

### Config Storage
- [src/infrastructure/config_store.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/config_store.rs:1)
- Loads and saves `config.json`.

### ProjectCatalog
- [src/infrastructure/project_catalog.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/project_catalog.rs:1)
- Persists the project registry.
- Normalizes/canonicalizes repo paths.
- Adds existing repos.
- Creates new repos and optional GitHub remotes.

### TaskStore
- [src/infrastructure/task_store.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/task_store.rs:1)
- Persists `TODO.md` and `CAPTAINS_LOG.md`.
- Owns task CRUD plus per-task session metadata updates.

### TODO Format
- [src/infrastructure/todo_format.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/todo_format.rs:1)
- Renders and parses embedded task metadata inside `TODO.md`.

### Captain's Log Format
- [src/infrastructure/captains_log_format.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/captains_log_format.rs:1)
- Renders and parses embedded summary metadata inside `CAPTAINS_LOG.md`.

### StateHistory
- [src/infrastructure/state_history.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/state_history.rs:1)
- Best-effort `.youbot` git snapshotting.
- Kept separate from storage so write durability and history policy are not the same concern.

### State Files
- [src/infrastructure/state_files.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/state_files.rs:1)
- Atomic local-file writes.
- Corrupt-file quarantine helpers.

### Tmux
- [src/infrastructure/tmux.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/tmux.rs:1)
- `TerminalSessionOps`
- `TmuxTerminal`

### Notifications
- [src/infrastructure/notification.rs](/Users/tombedor/development/youbot-rs/src/infrastructure/notification.rs:1)
- `NotificationSink`
- `SystemNotifier`

## UI

The TUI is feature-first rather than MVC-first. Each screen lives under its own module:

- `src/ui/home/`
- `src/ui/project_detail/`
- `src/ui/task/`
- `src/ui/add_repo/`
- `src/ui/live_session/`

Each feature module contains:

- `handler.rs`
  Key handling and route-local UI state transitions.
- `view.rs`
  Ratatui rendering for that feature.

The add-repo flow is a sequential wizard, not a hotkey-driven form. The current step enum and transient form state live in [src/ui/state.rs](/Users/tombedor/development/youbot-rs/src/ui/state.rs:1).
