# ADR 002: Repo Integration Contract

## Status

Accepted

## Context

Some repos integrated into youbot may be read-only, externally managed, or only lightly structured. Requiring repo-local governance files such as `PRD.md`, `AGENTS.md`, and `CAPTAINS_LOG.md` for all integrations would make onboarding unnecessarily difficult and would exclude repos that youbot should still be able to use.

## Decision

The minimum contract for integrating a repo into youbot is:

- a usable `justfile`

Everything else is optional for integrated repos. Youbot will maintain its own metadata registry for repo descriptions, tags, routing hints, preferred commands, and adapter configuration.

Repos created by youbot or explicitly adopted into managed mode must satisfy the stricter managed-repo standard described in `docs/PRD.md`.

## Consequences

Positive:
- Existing repos are easy to onboard
- Read-only repos remain usable
- Governance requirements are applied only where youbot has edit authority

Negative:
- Routing quality may initially be weaker for repos with sparse metadata
- Youbot must own more supplemental metadata itself

## Rejected alternatives

### Require `PRD.md`, `AGENTS.md`, and related docs for every integrated repo

Rejected because it assumes edit access and imposes unnecessary friction on external repos.

### No contract at all

Rejected because youbot still needs a stable execution surface. The `justfile` is the canonical minimal capability boundary.
