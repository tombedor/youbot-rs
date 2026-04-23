# Rust Library Choices

## Purpose

This document records the Rust-specific library choices for the first implementation of youbot. These choices are implementation details for the orchestrator itself; they do not change the language-agnostic integration contract for child repos.

## Selected libraries

### Terminal UI

- `ratatui`
- `crossterm`

Reasoning:
- `ratatui` provides the core layout, widgets, and rendering model for a Rust-native TUI.
- `crossterm` provides portable terminal setup, input events, and alternate-screen handling.
- This pair replaces the old Textual-oriented assumptions in the original spec.

### Serialization and config/state persistence

- `serde`
- `serde_json`

Reasoning:
- The spec already prefers JSON for v1 config.
- These crates are the standard low-friction choice for config and state records in Rust.

### Error handling

- `anyhow`

Reasoning:
- The application needs pragmatic top-level error propagation across config loading, filesystem work, parsing, and command execution.
- Domain-specific errors can be introduced later if a stricter error model becomes necessary.

### Time handling

- `chrono`

Reasoning:
- The state model stores timestamps in persisted records and log-style entries.
- `chrono` is sufficient for ISO timestamp generation and parsing in v1.

### Home/state root discovery

- `dirs`

Reasoning:
- The app needs a stable `~/.youbot/` state root on user machines.
- `dirs` keeps that path resolution simple and cross-platform.

### Process execution

- Rust standard library `std::process`

Reasoning:
- V1 command execution needs deterministic subprocess invocation of `just` and coding-agent backends.
- The standard library is enough before live streaming and async process supervision become necessary.

## Deferred libraries

These are intentionally not required for the first slice, but are likely later additions.

### Async runtime

- Candidate: `tokio`

Use when:
- OpenAI orchestration, coding-agent log streaming, or long-running background jobs need structured async concurrency.

### SQLite persistence

- Candidate: `rusqlite`

Use when:
- JSON files become too awkward for queryable state, append-heavy histories, or transactional updates.

### OpenAI integration

- Candidate: official OpenAI Rust SDK if available and suitable at implementation time, otherwise direct HTTP client usage

Use when:
- Milestone 5 primary chat orchestration is implemented.

### Structured tracing

- Candidate: `tracing`, `tracing-subscriber`

Use when:
- The app needs richer internal diagnostics beyond basic persisted run history.

## Non-goals

- No child-repo language adapters are required for integration.
- No repo-local Rust code is required in integrated repos.
- The orchestrator remains centered on the `justfile` contract, not the child repo's implementation language.
