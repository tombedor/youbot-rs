# Modules necesary


## Configuration / state management

### AppConfig
global config for any app-wide configuration

### TaskRepository
depends on: ProjectRegistry (for commiting changes)

- CRUD operations on TODO.md

### ProjectRegistry
- tracks which apps are integrated, what their config is for auto-merge vs open pr etc
- handles commits to local .youbot repo

## Session handlers

### TmuxClient
- low level tmux operations
- opens, creates, lists tmux sessions


### SessionManager
depends on: TmuxClient for low level Tmux operations
depends on: CodingAgentSupervisor
depends on: Notifier

- uses tmux `monitor-silence` with per-coding agent configuration to detect activity or idleness of background coding agents
- if background session is completed or stuck, dispatches Notifier and CodingAgentSupervisor for post-session tasks
- if background session needs input, dispatches CodingAgentSupervisor
- knows which sessions are active or inactive

## Agents

### Coding Agent Supervisor
depends on: TaskRepository for CRUD ops on tasks

- trigger by a session going inactive, or a live coding session being exited

- provides input to coding agent for background tasks
- evaluates whether task is done for background agents
- on exit, reviews transcript, writes summary to captains log, updates task status
- classifies call to summarize what a tasks title should be.




## Notifier


## TUI views
- each view has a controller, follow standard MVC architecture. Views are under a views/ , controllers under controllers/

  - HomeView
  - ProjectDetailView
  - TaskView
  - LiveCodingSessionView
  - AddRepoView




