# Milestones

## Purpose

This document defines the recommended implementation order. Each milestone should produce a usable vertical slice and should be completable without speculative work on later milestones.

## Milestone 1: Repo registry and command discovery

Goal:
- Register repos, validate `justfile`, and parse commands into canonical records.

Includes:
- config loading for registered repo paths
- repo validation
- `justfile` parsing
- in-memory or persisted registry storage

Done when:
- youbot can load the configured repos at startup
- repos without a usable `justfile` are rejected or marked invalid
- discovered commands are visible through a simple debug output or test

Non-goals:
- TUI rendering
- routing
- session persistence

## Milestone 2: Conversation and coding-agent continuation state

Goal:
- Persist youbot conversation history and repo-specific coding-agent session references.

Includes:
- conversation store
- coding-agent session registry
- append/read operations
- startup restore behavior
- active repo tracking

Done when:
- a user interaction can be recorded in youbot conversation history
- a repo can store a coding-agent session reference
- restarting the app restores conversation history and coding-agent session references

Non-goals:
- reconstructing full coding-agent transcripts
- session branching UI

## Milestone 3: Minimal terminal UI shell

Goal:
- Bring up the TUI with repo switching and a conversation pane.

Includes:
- app shell
- repo sidebar
- active repo selection
- basic conversation display

Done when:
- the user can see the configured repos
- switching repos changes active focus and available commands
- stored conversation history is rendered

Non-goals:
- routing
- structured data views

## Milestone 4: Command execution path

Goal:
- Execute selected `just` commands inside the active repo and render results.

Includes:
- executor
- basic command palette integration
- result capture and display

Done when:
- a repo command can be launched from the UI
- stdout, stderr, and exit status are surfaced
- execution events are appended to the relevant session

Non-goals:
- natural-language routing
- adapter rendering beyond basic text

## Milestone 5: Primary chat orchestration

Goal:
- Route natural-language requests through the primary chat orchestrator to repo + action + command.

Includes:
- OpenAI Responses API integration
- tool definition and tool execution loop
- concise final answer generation
- fallback behavior for low confidence or unavailable API

Done when:
- a natural-language request selects the correct repo in common cases
- command requests execute without manual command selection
- low-confidence routes surface a clarification path instead of guessing blindly

Non-goals:
- code-change flow
- learned routing improvements

## Milestone 6: Code-change flow

Goal:
- Invoke a configurable coding-agent backend inside a repo when no existing command satisfies the request.

Includes:
- backend abstraction
- subprocess integration
- request/context handoff
- result capture

Done when:
- a routed `code_change` action launches the configured backend in the repo directory
- switching between Claude Code and Codex is a config change
- completion or failure is surfaced in the conversation pane

Non-goals:
- automatic post-change command generation

## Milestone 7: Adapter-backed views

Goal:
- Introduce youbot-owned adapters that render structured repo views.

Includes:
- adapter storage
- adapter loader
- command-output parsing rules
- repo-specific screens or view modes

Done when:
- an adapter can contribute repo-scoped command-palette actions
- at least one repo can display a structured view derived from command output

Non-goals:
- arbitrary plugin execution from child repos

## Milestone 8: Scheduling

Goal:
- Execute configured recurring commands and surface their results.

Includes:
- scheduler config
- recurring job runner
- recent job status display

Done when:
- a scheduled `just` command runs without manual intervention
- the result is stored and visible in the UI

## Milestone 9: Managed repo scaffolding

Goal:
- Initialize a new managed repo with the stricter standard.

Includes:
- scaffold generation
- required docs
- `Cargo.toml`
- baseline `justfile`

Done when:
- a new managed repo can be created from the UI or CLI
- the generated repo matches the managed repo standard in `docs/PRD.md`

## Suggested implementation discipline

- Finish milestones in order unless a later milestone is blocking a basic earlier design choice.
- Keep each milestone shippable.
- Write acceptance checks from `docs/acceptance.md` as each milestone lands.
- Avoid inventing persistence or interface shapes that contradict `docs/interfaces.md`.
