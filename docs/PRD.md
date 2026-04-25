# Product Notes

## State Model

The installed `youbot` instance keeps its own state repo at `~/.youbot/`. This is separate from any integrated user repo.

Persisted state includes:

- app config
- integrated project registry
- per-project `TODO.md`
- per-project `CAPTAINS_LOG.md`
- persisted session records

For each project:

- tasks have `title`, `description`, and `status`
- tasks can have at most one session per `coding-agent product + session kind`
- inactive sessions may carry a summary
- each project stores its merge policy: auto-merge or open PR

## Coding Agents

Agents in scope:

- codex
- claude code

## UI

All current screens are TUI screens implemented under `src/ui/`.

### Home

- lists projects
- shows latest session for the selected project
- shows active background sessions for the selected project

Actions:

- select a project
- enter project detail
- attach to the selected project’s active background session
- enter add-repo wizard

### Add Repo

The add-repo flow is a sequential wizard. The user answers one question at a time.

Questions:

1. Is this an existing repo or a new repo?
2. If existing: what is the repo path?
3. If new: what is the repo name?
4. If new: where should the repo be created?
5. If new: should this location become the default?
6. If new: what language template should be initialized?
7. If new: should a GitHub remote be created?
8. For both new and existing repos: should this project auto-merge or open PRs?

Typing should only answer the current text field. Choice steps use left/right plus enter.

### Project Detail

- shows task list and task statuses
- allows task creation from description
- allows task-status cycling
- allows project merge-mode toggling
- allows opening task detail
- allows attaching to the task’s background codex session

### Task Detail

- shows task description
- shows prior sessions and summaries

Actions:

- start/resume live codex session
- start/resume background codex session
- attach to background codex session
- start/resume live claude session

### Live Session

- tmux owns the live interactive experience
- on return from attach, the app re-enters the TUI, finalizes the session, refreshes state, and routes back home

## Background Sessions

Background sessions run in tmux and are polled by the app.

Expected behavior:

- if the agent is waiting for user input, prompt it to continue autonomously when possible
- if the agent completes or gets stuck, persist updated state and send an OS notification
- summaries and status updates are written back into task state and captain’s log
