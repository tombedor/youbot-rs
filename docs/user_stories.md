# User Stories

## Purpose

This document captures the user-driven interactions that youbot must support. It complements `docs/PRD.md` by describing concrete user goals and expected product behavior, and it complements `docs/acceptance.md` by framing those behaviors from the user's perspective.

These stories should be treated as implementation-facing requirements for the TUI, routing, execution, and coding-agent continuation model.

## Core conversational stories

### 1. Ask a repo-specific question in plain language

As a user, I want to ask a question like "what's on my to-do list?" without knowing the command name <!-- or even, possibly, the relevant repo -->, so that I can use repos through natural language.

Expected behavior:
- youbot selects the most relevant repo
- youbot runs the appropriate command or query path
- the result is shown inline in the conversation pane

### 2. Refine a previous result conversationally

As a user, I want to say things like "those openings, remote only" after a prior result, so that I can continue working without restating context.

Expected behavior:
- the follow-up uses recent conversation context plus repo focus
- prior result context is available to routing
- the updated result replaces or follows the prior result clearly

### 3. Use a global conversation for cross-repo requests

As a user, I want to ask cross-repo questions without forcing a single repo context, so that I can treat youbot as an orchestrator rather than just a repo launcher.

Expected behavior:
- a global scope exists alongside repo scopes
- the app opens in global chat with no repo preselected
- global requests can route across repos or ask clarifying questions
- global context remains available without requiring a separate repo transcript

### 4. Get a clarification instead of an incorrect action

As a user, I want youbot to ask when a request is ambiguous, so that it does not confidently do the wrong thing.

Expected behavior:
- low-confidence routing produces a clarification step
- no command is executed until ambiguity is resolved

### 5. Know when youbot is still working

As a user, I want a visible processing indicator after I send a message, so that I can distinguish active work from a stalled UI.

Expected behavior:
- the chat area shows an in-flight indicator while a request is running
- the indicator disappears when a result or error is rendered

### 6. Watch coding-agent work as it happens

As a user, I want to see live logs and session status when a coding-agent run starts, so that I can tell whether the agent has launched, what it is doing, and whether it is still making progress.

Expected behavior:
- the UI shows that a coding session has started
- the active backend and target are visible
- incremental agent output or log events appear while the run is in progress
- the live activity view remains associated with the resulting conversation entry

## Navigation and scope stories

### 7. See all available repos and their state

As a user, I want to see the repos youbot knows about and whether they are usable, so that I understand what the system can act on.

Expected behavior:
- the sidebar shows registered repos
- the sidebar can be dismissed without losing chat context
- degraded or invalid repos are clearly distinguishable
- repo status does not require leaving the main UI

### 8. Switch the active repo quickly

As a user, I want to move between repos quickly, so that I can work across several domains in one interface.

Expected behavior:
- switching repos changes active scope immediately
- the conversation pane remains coherent while repo focus changes
- only one repo panel is visible at a time for the active repo
- if no repo is active, no repo panel is shown
- repo-specific commands update with focus

### 9. Resume an existing coding-agent session for a repo

As a user, I want code-change work in a repo to continue the existing Claude Code or Codex session when possible, so that the coding agent keeps its own native working context.

Expected behavior:
- youbot stores backend-native session references per repo
- code-change work resumes the backend's own session rather than reconstructing history in youbot
- youbot uses only automation-compatible non-interactive continuation modes
- repo focus helps select the right coding-agent session

## Command and execution stories

### 10. Discover executable capabilities through the command palette

As a user, I want to see the commands available for the current repo, so that I can use direct execution when that is faster than natural language.

Expected behavior:
- global commands are always visible
- repo-scoped commands appear only for the active repo
- command names are understandable enough to invoke intentionally

### 11. Run a known repo command directly

As a user, I want to trigger a known action directly, so that repetitive tasks are efficient.

Expected behavior:
- selecting a command runs it in the correct repo directory
- stdout, stderr, and exit status are surfaced clearly
- the result becomes part of visible youbot interaction history

### 12. Inspect failures without losing context

As a user, I want failed commands to be visible and non-destructive, so that I can recover from errors without restarting my work.

Expected behavior:
- failures are rendered inline
- raw error output remains accessible
- the app stays usable after a command failure

## Repo onboarding and management stories

### 13. Add an existing repo with minimal friction

As a user, I want to add a repo that already exists on disk as long as it has a `justfile`, so that youbot can work with repos I do not control deeply.

Expected behavior:
- onboarding requires only a valid path and a usable `justfile`
- onboarding persists the repo into youbot config
- onboarding makes the repo available immediately without manual config edits
- missing governance docs do not block integration
- initial repo metadata can live in youbot state

### 14. Supply richer metadata for a lightly documented repo

As a user, I want to add a summary, tags, and preferred commands for a repo, so that routing quality improves even when the repo itself is sparse.

Expected behavior:
- metadata is stored by youbot, not forced into the child repo
- metadata can be updated without changing the repo

### 15. Re-scan a repo after its commands change

As a user, I want youbot to refresh its command inventory, so that the UI and routing stay current when a repo evolves.

Expected behavior:
- re-scan updates command records
- re-scan regenerates adapter metadata when overview commands or rendering hints should change
- stale commands are removed or marked stale
- refreshed commands appear in the palette and routing context

### 16. Generate an adapter when onboarding a repo

As a user, I want onboarding to generate a usable initial adapter automatically, so that a newly integrated repo gets a meaningful workspace without manual UI coding.

Expected behavior:
- onboarding writes adapter metadata into youbot state
- onboarding generates an initial overview command choice for the selected-repo workspace
- generated adapter artifacts can be refined later without requiring child-repo changes

### 17. Initialize a new managed repo

As a user, I want youbot to scaffold a new repo when I identify a new capability area, so that new tools start from a consistent standard.

Expected behavior:
- managed repo scaffolding creates the required files and baseline commands
- the new repo is registered into youbot after creation

## Change-request stories

### 18. Request a code change when no command exists

As a user, I want to ask for a new capability in plain language, so that I do not need to manually open a repo and script the change myself.

Expected behavior:
- youbot checks for an existing command first
- if none fits, youbot routes to the code-change flow
- the result is reported back in the conversation

### 19. Switch coding-agent backends without reworking the system

As a user, I want to switch between Claude Code and Codex as the coding agent, so that I can choose the backend that fits the task or my environment.

Expected behavior:
- backend selection is configured by youbot
- the rest of the system does not need to change when the backend changes
- a repo may optionally override the global default backend

### 20. Promote repeated work into a first-class command

As a user, I want frequently repeated actions to become explicit commands, so that the system gets more efficient over time.

Expected behavior:
- after a successful code change, youbot can suggest or add a `just` command
- future requests can take the command path directly

### 21. Change a repo's youbot view without editing the child repo

As a user, I want requests about how a repo looks inside youbot to change the adapter/view layer by default, so that UI tweaks do not accidentally mutate the child repo.

Expected behavior:
- a request like "change the life_admin view to show overdue tasks first" targets the youbot-owned adapter unless I explicitly ask to change `life_admin`'s own code or output
- if the system cannot tell whether I mean the adapter or the child repo, it asks for clarification before making changes
- after an adapter/view change, the affected workspace reloads so the updated view is visible immediately

## Structured view stories

### 22. See structured data in a dedicated view mode

As a user, I want results like job listings or task lists to render as structured views rather than only plain text, so that scanning and comparison are easier.

Expected behavior:
- adapters can render tables, lists, or repo-specific panels
- structured views are owned by youbot, not child repos

### 23. Apply UI-only filters to current results

As a user, I want to filter or sort visible results without necessarily re-running a command, so that exploration is fast.

Expected behavior:
- some adapter actions can operate on current displayed data
- these actions are available only when relevant to the current repo/view

### 24. See repo-specific quick actions when a repo is focused

As a user, I want a selected repo workspace to surface a few recommended actions immediately, so that I can act without scanning a long generic command list.

Expected behavior:
- the selected-repo workspace shows a compact set of adapter-defined quick actions
- the workspace header identifies the selected repo and its purpose
- the workspace emphasizes repo-specific data over raw command inventory

## Scheduling stories

### 25. Configure recurring repo actions centrally

As a user, I want recurring tasks like nightly searches to run from youbot, so that scheduling is centralized and not duplicated inside child repos.

Expected behavior:
- schedules are configured in youbot state
- scheduled runs execute in the correct repo
- recent run results are visible in the UI

### 26. Review scheduled run outcomes

As a user, I want to inspect what happened in scheduled runs, so that background automation remains observable.

Expected behavior:
- scheduled success and failure states are persisted
- the UI exposes recent scheduled activity

## Degraded-state stories

### 27. Keep using other repos when one repo is broken

As a user, I want one missing or failing repo not to take down the whole tool, so that the orchestrator remains useful under partial failure.

Expected behavior:
- repo failures are isolated
- unaffected repos remain usable

### 28. Understand why a repo cannot be used

As a user, I want a broken repo to show a clear reason, so that I can fix the problem without guessing.

Expected behavior:
- invalid path, missing `justfile`, and command failures are surfaced distinctly

## Prioritization

The highest-priority user stories for v1 are:

1. Ask a repo-specific question in plain language
2. Switch the active repo quickly
3. Resume an existing coding-agent session for a repo
4. Run a known repo command directly
5. Add an existing repo with minimal friction
6. Generate an adapter when onboarding a repo
7. Request a code change when no command exists
8. Switch coding-agent backends without reworking the system
9. See structured data in a dedicated view mode
10. Configure recurring repo actions centrally

## Relationship to other docs

- `docs/PRD.md` defines product scope and high-level behavior
- `docs/architecture.md` defines implementation structure
- `docs/interfaces.md` defines canonical internal records
- `docs/milestones.md` defines build order
- `docs/acceptance.md` defines testable correctness checks

If a future design choice satisfies the architecture but fails an important user story here, the user story should take precedence unless explicitly superseded.
