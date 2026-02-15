---
sidebar_position: 5
---

# Workdir and Worktree Strategy

Choosing the right run location is the biggest lever for stable agent behavior.

## Two Modes

- `use_existing`: run directly in an existing directory
- `create_worktree`: create an isolated worktree under your configured root

## How to Choose

Use `use_existing` when:

- You need direct access to a long-lived local workspace
- You intentionally want local uncommitted state to be visible

Use `create_worktree` when:

- You want reproducible isolated runs
- Multiple tasks may run in parallel on the same repository
- You need safer cleanup boundaries after task completion

## Naming Convention Suggestion

Use predictable names for easier operations:

- Agent name: `<repo>-<goal>-<date>`
- Worktree path suffix: `<ticket-or-topic>-<short-hash>`

Example:

- `agent`: `agenthub-docs-2026-02-15`
- `worktree`: `docs-user-guide-5db671b0`

## Safe Path Constraints

All run paths must be under configured `safe_paths`.

If creation fails with path-related errors:

1. Confirm base path is allowed
2. Avoid symlink-heavy paths at first
3. Retry with a shorter explicit directory path

## Cleanup Practice

For `create_worktree` mode:

1. Keep each task focused
2. Delete stale task agents regularly
3. Archive or merge useful changes before cleanup
