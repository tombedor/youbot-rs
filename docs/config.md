# Configuration

## Purpose

This document defines the user-editable configuration for youbot. It should be possible to understand how the app is wired without reading code, and implementation should not invent additional config surfaces unless they are clearly warranted.

## Principles

- Config is user-owned input.
- Youbot may update config as part of explicit onboarding flows such as repo registration.
- Runtime state is not stored in config.
- Config should be small, explicit, and stable.
- Repo metadata that is learned or cached over time belongs in state, not in config.
- Backend-native coding-agent session ids belong in state, not in config.
- Primary chat credentials belong in environment variables, not in config.
- Developer review artifacts and usage bundles belong in state, not in config.

## Config location

V1 should use a single primary config file in the youbot state root:

```text
~/.youbot/config.json
```

JSON is preferred for v1 because it is easy to inspect, edit, and load without adding another parser dependency.

If a future version needs comments or richer ergonomics, TOML can be considered later. Do not block implementation on that decision.

## Top-level config shape

Suggested shape:

```json
{
  "repos": [],
  "scheduler": {
    "enabled": false,
    "jobs": []
  },
  "coding_agent": {
    "default_backend": "codex",
    "backends": {}
  }
}
```

## Repo entries

`repos` is the authoritative list of integrations youbot should attempt to load at startup.

This list may be edited by the user directly or updated by youbot when a repo is added through the onboarding flow.

Suggested shape:

```json
{
  "repo_id": "job_search",
  "name": "job_search",
  "path": "/Users/tombedor/development/job_search",
  "classification": "integrated"
}
```

Required fields:
- `repo_id`
- `name`
- `path`
- `classification`

Rules:
- `path` must point to the repo root
- `classification` is either `integrated` or `managed`
- `repo_id` must be stable once created

Config should not need to duplicate learned metadata like tags, preferred commands, or routing hints. Those belong in state.

## Scheduler config

Suggested shape:

```json
{
  "enabled": true,
  "jobs": [
    {
      "job_id": "job_search_nightly",
      "repo_id": "job_search",
      "command_name": "pipeline-status",
      "schedule_type": "cron",
      "cron": "0 21 * * *"
    }
  ]
}
```

Required fields per job:
- `job_id`
- `repo_id`
- `command_name`
- `schedule_type`

V1 schedule types:
- `cron`

Optional future schedule types:
- `interval`
- `manual_only`

## Coding-agent config

The coding-agent backend must be selectable without code changes.

Suggested shape:

```json
{
  "default_backend": "codex",
  "backends": {
    "codex": {
      "command_prefix": ["codex"],
      "default_args": []
    },
    "claude_code": {
      "command_prefix": ["claude"],
      "default_args": []
    }
  }
}
```

Required fields:
- `default_backend`
- `backends`

Rules:
- `default_backend` must be one of the configured backend keys
- each backend must declare `command_prefix`
- repo-specific backend overrides are stored in state metadata, not necessarily in config

## Environment variables

Primary chat uses the OpenAI API.

Required:
- `OPENAI_API_KEY`

Optional:
- `OPENAI_MODEL`

These are process environment settings rather than file-based config.

## Example full config

```json
{
  "repos": [
    {
      "repo_id": "job_search",
      "name": "job_search",
      "path": "/Users/tombedor/development/job_search",
      "classification": "integrated"
    },
    {
      "repo_id": "life_admin",
      "name": "life_admin",
      "path": "/Users/tombedor/development/life_admin",
      "classification": "integrated"
    },
    {
      "repo_id": "trader-bot",
      "name": "trader-bot",
      "path": "/Users/tombedor/development/trader-bot",
      "classification": "integrated"
    }
  ],
  "scheduler": {
    "enabled": false,
    "jobs": []
  },
  "coding_agent": {
    "default_backend": "codex",
    "backends": {
      "codex": {
        "command_prefix": ["codex"],
        "default_args": []
      },
      "claude_code": {
        "command_prefix": ["claude"],
        "default_args": []
      }
    }
  }
}
```

## Non-goals

- No secrets should be stored in youbot config in v1.
- No learned routing hints should be written into config.
- No command inventory should be duplicated in config.
