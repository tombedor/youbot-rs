# ADR 001: Conversation Model

## Status

Accepted

## Context

Youbot is a conversational orchestrator spanning multiple repos. The first design iteration treated this as a global session plus one youbot session per repo. That turned out to conflate youbot conversation with coding-agent session continuation.

## Decision

Youbot will keep lightweight conversation history for orchestration and routing, but it will not model repo-specific persistent chat sessions as a first-class concept in v1.

Repo-specific continuation is handled separately through backend-native coding-agent session references.

## Consequences

Positive:
- Youbot conversation stays simple
- Coding-agent continuation is modeled explicitly rather than implicitly
- Repo focus still works without forcing a transcript per repo

Negative:
- Some repo-specific routing context may rely on recent history plus repo focus rather than a dedicated transcript

## Rejected alternatives

### Global plus per-repo youbot sessions

Rejected because it overloaded the meaning of "session" and did not match the real requirement, which was mainly coding-agent continuation.
