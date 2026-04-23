# Interfaces

## Purpose

This document defines the canonical internal records and service interfaces for youbot. An implementation should use these interfaces to avoid drifting into incompatible ad hoc structures.

## Canonical records

These records may be implemented as Rust structs and enums, JSON-backed schema types, or similar typed structures. The important requirement is that the field meanings remain stable.

Type notation below is language-neutral:

- `string`
- `integer`
- `float`
- `boolean`
- `timestamp`
- `list<T>`
- `optional<T>`
- `enum[...]`
- `object`

### `RepoRecord`

Represents one registered integration target.

Required fields:

```text
repo_id: string
name: string
path: string
classification: enum["integrated", "managed"]
status: enum["ready", "invalid", "missing", "error"]
```

Optional fields:

```text
purpose_summary: optional<string>
tags: list<string>
preferred_commands: list<string>
last_scanned_at: optional<timestamp>
last_active_at: optional<timestamp>
adapter_id: optional<string>
```

Invariants:
- `repo_id` is stable across restarts
- `path` points to the repo root
- `classification` drives policy, not capability

### `CommandRecord`

Represents one discovered `just` recipe.

```text
repo_id: string
command_name: string
display_name: string
description: optional<string>
invocation: list<string>
supports_structured_output: boolean
structured_output_format: enum["json", "text", "unknown"]
tags: list<string>
```

Invariants:
- `invocation` is executable from the repo root
- `command_name` is unique within a repo

### `ConversationMessage`

Represents one item in youbot's own conversation history.

```text
message_id: string
role: enum["user", "assistant", "system", "tool"]
content: string
created_at: timestamp
```

Invariants:
- Messages belong to youbot's orchestration conversation, not to backend-native coding-agent transcripts

### `ConversationRecord`

Represents persisted youbot conversation history.

```text
conversation_id: string
messages: list<ConversationMessage>
updated_at: timestamp
last_response_id: optional<string>
```

### `UsageReviewBundle`

Represents a bounded developer-facing snapshot of how this installation of youbot has been used.

```text
bundle_id: string
created_at: timestamp
source_state_root: string
window_summary: string
conversation_id: optional<string>
messages: list<ConversationMessage>
command_runs: list<object>
coding_agent_runs: list<object>
activity_entries: list<object>
activity_log_refs: list<string>
notes: list<string>
```

Invariants:
- A bundle is derived review data, not the primary source of truth
- A bundle must be bounded in size and time window
- A bundle is intended for review of the `youbot` repo itself, not as ambient context for arbitrary child-repo coding work

### `RouteDecision`

The router's structured output.

```text
repo_id: optional<string>
action_type: enum["command", "query", "code_change", "adapter_change", "clarify", "global_action"]
command_name: optional<string>
arguments: list<string>
reasoning_summary: string
confidence: float
```

Invariants:
- `command_name` is required when `action_type == "command"`
- `repo_id` may be absent only for global actions or clarifications
- `confidence` is normalized to `0.0 <= x <= 1.0`
- `action_type == "adapter_change"` targets youbot-owned adapter/view behavior rather than child-repo code

### `CodingAgentBackend`

Represents the configured backend for code-change work.

```text
backend_name: enum["claude_code", "codex"]
command_prefix: list<string>
default_args: list<string>
```

Invariants:
- `command_prefix` is executable on the host
- callers do not branch on backend-specific shell details outside the runner

### `ExecutionResult`

Represents a completed command execution.

```text
repo_id: string
command_name: string
invocation: list<string>
exit_code: integer
stdout: string
stderr: string
started_at: timestamp
finished_at: timestamp
duration_ms: integer
parsed_payload: optional<object>
```

### `CodingAgentResult`

Represents a completed coding-agent invocation.

```text
repo_id: string
backend_name: enum["claude_code", "codex"]
exit_code: integer
stdout: string
stderr: string
started_at: timestamp
finished_at: timestamp
duration_ms: integer
```

### `CodingAgentSessionRef`

Represents a backend-native resumable coding-agent session for a repo.

```text
repo_id: string
backend_name: enum["claude_code", "codex"]
session_kind: enum["noninteractive"]
session_id: string
purpose_summary: optional<string>
status: enum["active", "stale", "unknown"]
last_used_at: timestamp
```

Invariants:
- `session_id` is a backend-native continuation handle
- `session_kind` is automation-compatible and non-interactive for youbot-driven runs
- youbot stores the handle and minimal metadata, not the full coding-agent transcript

### `AdapterRecord`

Represents a youbot-owned adapter for a repo.

```text
adapter_id: string
repo_id: string
version: string
view_names: list<string>
command_palette_entries: list<string>
output_rules: list<string>
updated_at: timestamp
overview_sections: list<OverviewSectionSpec>
quick_actions: list<QuickActionSpec>
```

### `OverviewSectionSpec`

Represents generated adapter metadata for the selected-repo overview panel.

```text
command_name: string
arguments: list<string>
title: optional<string>
max_lines: integer
fallback_command_names: list<string>
render_mode: enum["json", "text"]
```

### `QuickActionSpec`

Represents a recommended action for the selected-repo workspace.

```text
command_name: string
title: optional<string>
arguments: list<string>
```

## Service interfaces

These are logical interfaces. Implementations may vary, but the contracts should remain intact.

### `Registry`

Responsibilities:
- CRUD for repo metadata
- Store and retrieve command inventory
- Store routing hints and adapter references

Suggested methods:

```text
register_repo(path, name?) -> RepoRecord
get_repo(repo_id) -> RepoRecord
list_repos() -> list<RepoRecord>
update_repo(repo) -> RepoRecord
store_commands(repo_id, commands) -> void
list_commands(repo_id) -> list<CommandRecord>
```

Behavioral rules:
- `register_repo(...)` persists the repo into user config
- registration triggers command discovery and adapter generation
- integrated repo registration requires only a valid path and `justfile`

### `ConversationStore`

Responsibilities:
- Append and read youbot conversation messages
- Track the provider-native response id for OpenAI conversation continuation

Suggested methods:

```text
load() -> ConversationRecord
append(message) -> void
set_last_response_id(response_id) -> void
truncate(max_messages) -> void
```

Behavioral rules:
- The store persists only youbot conversation state
- It must not be treated as the source of truth for backend-native coding-agent context

### `CodingAgentSessionRegistry`

Responsibilities:
- Store and retrieve backend-native session references by repo

Suggested methods:

```text
get(repo_id) -> optional<CodingAgentSessionRef>
set(session_ref) -> void
remove(repo_id) -> void
list() -> list<CodingAgentSessionRef>
```

Behavioral rules:
- Stores handles and summary metadata only
- Does not persist full coding-agent transcripts

### `JustfileParser`

Responsibilities:
- Parse a repo's `justfile`
- Normalize discovered recipes into `CommandRecord` values

Suggested methods:

```text
parse(repo_id, repo_root) -> list<CommandRecord>
```

Behavioral rules:
- Parsing should be deterministic
- Unknown recipe details may fall back to minimal records rather than failing the whole repo

### `Executor`

Responsibilities:
- Run repo commands
- Capture stdout, stderr, exit code, timestamps, and structured output when available

Suggested methods:

```text
run(repo, command_name, arguments) -> ExecutionResult
```

Behavioral rules:
- Execution occurs in the repo root
- Structured parsing failure must not hide raw output

### `CodingAgentRunner`

Responsibilities:
- Invoke the configured coding-agent backend for code-change requests
- Reuse backend-native session continuation when possible
- Publish incremental activity records for the UI

Suggested methods:

```text
run_code_change(repo, request) -> CodingAgentResult
resume(repo, session_ref, request) -> CodingAgentResult
```

Behavioral rules:
- Uses automation-compatible, non-interactive backend entrypoints only
- Handles backend-specific continuation details inside the runner

### `OpenAIChat`

Responsibilities:
- Call the OpenAI Responses API for primary orchestration
- Expose tool definitions and drive the tool loop
- Persist provider-native response continuation ids

Suggested methods:

```text
respond(user_message, context, tools, previous_response_id?) -> { assistant_text, response_id }
```

Behavioral rules:
- Primary orchestration is tool-driven
- A local router may exist as fallback when OpenAI-backed orchestration is unavailable

### `Router`

Responsibilities:
- Provide a local fallback route decision for simple prompts

Suggested methods:

```text
route(message, context) -> RouteDecision
```

Behavioral rules:
- This is fallback behavior, not the primary orchestration path

### `AdapterStore`

Responsibilities:
- Load, store, and refresh youbot-owned adapter metadata

Suggested methods:

```text
get(repo_id) -> optional<AdapterRecord>
set(adapter) -> void
refresh(repo_id) -> AdapterRecord
```

Behavioral rules:
- Adapters belong to youbot-owned state, not child repos
- Adapter refresh should be idempotent

## Intentionally deferred details

- Exact Rust crate/module boundaries
- Exact persistence engine choice for each store
- Exact TUI screen and widget breakdown
- Exact command-palette implementation strategy
