# ADR 005: Backend-Native Session Continuation

## Status

Accepted

## Context

Youbot needs to continue coding-agent work across invocations in repos such as `job_search`, `life_admin`, and `trader-bot`. A naive design would store transcripts inside youbot and attempt to reconstruct agent state on each code-change request.

That would duplicate the coding agent's own session model and create a fragile approximation of backend behavior.

## Decision

Youbot will use backend-native coding-agent continuation whenever the backend supports it.

For each repo, youbot stores only a minimal session reference such as:

- backend name
- session kind
- backend-native session id
- short purpose/summary
- last used timestamp

When continuing code-change work, youbot should call the coding agent's own continuation mechanism rather than rebuilding context from a stored transcript.

Youbot will only orchestrate automation-compatible, non-interactive coding-agent flows. Interactive session pickers and interactive terminal resume workflows are not part of the orchestration path.

Example:
- `claude --resume <SESSION_ID> --print "..."`
- `codex exec resume <SESSION_ID> "..."`

## Consequences

Positive:
- Youbot stays aligned with the backend's real session semantics
- Session continuation is simpler and less error-prone
- Youbot avoids becoming a second transcript manager for code agents
- Orchestration remains scriptable and deterministic

Negative:
- Continuation behavior is constrained by what each backend actually supports
- Youbot must handle backend-specific continuation flags and session-id discovery

## Rejected alternatives

### Reconstruct coding-agent sessions from youbot-side transcripts

Rejected because it duplicates backend behavior poorly and creates drift between youbot's model and the agent's actual session state.

### Treat each code-change request as a fresh agent invocation

Rejected because it loses valuable working context and makes ongoing repo work less effective.
