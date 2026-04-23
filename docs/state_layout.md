# State Layout

## Purpose

This document defines the on-disk layout for youbot-owned runtime state. It separates stable user config from mutable operational state such as sessions, discovered commands, adapter data, and execution history.

## State root

V1 should use:

```text
~/.youbot/
```

This directory is fully owned by youbot.

## Directory layout

Suggested layout:

```text
~/.youbot/
  config.json
  registry/
    repos.json
    commands.json
    routing_hints.json
  conversation/
    history.json
  coding_agent_sessions/
    sessions.json
  activity/
    coding_agent_current.json
    coding_agent_events.jsonl
  adapters/
    metadata/
      job_search.json
      life_admin.json
      trader-bot.json
    generated/
  scheduler/
    jobs.json
    run_history.json
  runs/
    commands.jsonl
    coding_agents.jsonl
  reviews/
    latest.json
    bundles/
```

This is a recommended v1 shape. The important constraint is consistency, not the exact filenames.

## Config versus state

`config.json`
- user-edited configuration
- repo registrations
- scheduler configuration
- coding-agent defaults

Everything else in the state root is mutable runtime state managed by youbot.

## Registry files

### `registry/repos.json`

Stores normalized repo metadata records.

Contents may include:
- repo id
- name
- path
- classification
- status
- purpose summary
- tags
- preferred commands
- adapter id
- timestamps

### `registry/commands.json`

Stores discovered command inventory by repo.

Contents may include:
- repo id
- command name
- display name
- description
- structured-output hints
- parse timestamps

### `registry/routing_hints.json`

Stores learned or user-supplied routing hints.

Contents may include:
- common prompt patterns
- preferred commands for common intents
- repo-specific keywords

This file is mutable and should not be treated as authoritative product logic.

## Conversation files

### `conversation/history.json`

Stores youbot's own conversation history.

This is for orchestration context and UI continuity. It is not intended to reconstruct backend-native coding-agent sessions.

Contents include:
- conversation id
- ordered message list
- updated timestamp
- `last_response_id` for continuing the provider-native OpenAI conversation

## Coding-agent session registry

### `coding_agent_sessions/sessions.json`

Stores backend-native coding-agent session references by repo.

Contents may include:
- repo id
- backend name
- session kind
- session id
- purpose summary
- status
- last used timestamp

This registry stores continuation handles for automation-compatible runs, such as a Codex non-interactive session id that can later be resumed via backend-native commands like `codex exec resume <UUID>`.

## Live activity files

### `activity/coding_agent_current.json`

Stores the current or most recent coding-agent activity snapshot for live UI rendering.

Contents may include:
- run id
- target repo id
- target kind (`repo` or `adapter`)
- backend name
- request summary
- session id
- status
- recent streamed entries

### `activity/coding_agent_events.jsonl`

Append-only record of coding-agent activity events.

Each line may contain:
- run id
- event kind such as `started`, `output`, `session_id`, or `finished`
- stream name for output events
- content
- timestamp

## Adapter files

### `adapters/metadata/<repo_id>.json`

Stores adapter metadata and rendering hints.

Contents may include:
- adapter id
- view names
- command palette entries
- output rules
- overview command metadata
- update timestamp

### `adapters/generated/`

Stores generated adapter artifacts created during onboarding or refresh.

Examples:
- generated notes about the selected overview command
- future generated parser or rendering scaffolds

## Scheduler files

### `scheduler/jobs.json`

Stores normalized scheduled jobs derived from config.

### `scheduler/run_history.json`

Stores recent scheduler execution outcomes.

Contents may include:
- job id
- repo id
- command name
- started/finished timestamps
- exit code
- brief summary

## Run logs

### `runs/commands.jsonl`

Append-only record of command executions.

Each line should contain:
- repo id
- command name
- invocation
- exit code
- timestamps

### `runs/coding_agents.jsonl`

Append-only record of coding-agent invocations.

Each line should contain:
- repo id
- backend name
- request summary
- exit code
- timestamps

These logs are useful for debugging and auditing, but they are not the primary source of truth for sessions.

## Review artifacts

### `reviews/latest.json`

Stores a pointer or lightweight summary for the most recent developer usage review.

Contents may include:
- bundle id
- created timestamp
- bundle path
- window summary

### `reviews/bundles/<bundle_id>.json`

Stores a bounded usage-review bundle for developer analysis of the `youbot` repo.

Contents may include:
- source state root
- reviewed time window summary
- recent conversation messages
- recent command-run records
- recent coding-agent-run records
- recent activity entries
- references to any live activity log files that were included
- summarization notes or trimming markers

These bundles are derived artifacts. They are intended to help the `youbot` coding agent review how the tool has been used without treating the full raw state as implicit background context for every coding session.

## Data ownership rules

- Config is user-owned input.
- Registry files are youbot-owned derived state.
- Conversation files are the source of truth for youbot conversational memory.
- Coding-agent session registry is the source of truth for backend-native continuation handles.
- Adapter files are the source of truth for TUI rendering hints.
- Run logs are append-only operational history.
- Review bundles are derived developer-facing artifacts, not source-of-truth state.

## Format choices

Recommended v1 formats:
- `json` for structured state snapshots
- `jsonl` for append-only event logs

Reasons:
- easy to inspect by hand
- easy to patch or recover
- no schema-migration machinery required for the first version

If the state becomes too complex later, SQLite can replace some or all of these stores behind the same interfaces.

## Recovery rules

- Missing state files should be recreated automatically.
- Corrupt per-repo state should not prevent loading other repos.
- Corrupt run logs should not block app startup.
- Config errors should be surfaced clearly because config is user-authored.
