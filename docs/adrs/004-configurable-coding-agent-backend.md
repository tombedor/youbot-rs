# ADR 004: Configurable Coding-Agent Backend

## Status

Accepted

## Context

Youbot needs a code-change path for requests that are not already covered by existing `just` commands. Hardcoding a single backend would make the orchestrator unnecessarily rigid and would force architectural changes whenever the preferred coding agent changes.

The immediate requirement is to support both Claude Code and Codex and make switching between them easy.

## Decision

Youbot will treat the coding agent as a configurable backend behind a stable internal runner interface.

Initial supported backends:

- Claude Code
- Codex

Backend selection will be configurable globally and may optionally be overridden per repo.

## Consequences

Positive:
- Switching coding agents becomes an operational choice rather than a refactor
- The rest of the system can depend on one stable code-change interface
- Repo-specific preferences can be supported without forking the architecture

Negative:
- The runner must absorb backend-specific invocation details
- Testing must cover at least two backend paths

## Rejected alternatives

### Hardcode Claude Code as the only backend

Rejected because it creates unnecessary lock-in and conflicts with the requirement to support Codex as well.

### Let callers invoke backends directly

Rejected because backend-specific shell details would leak across the codebase and make future changes harder.
