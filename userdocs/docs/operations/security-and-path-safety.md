---
sidebar_position: 8
---

# Security and Path Safety

This page explains practical guardrails for safe day-to-day usage.

## Principle: Least Path Access

Only allow paths users actually need.

Good:

- `/home/you/projects/agenthub`
- `/home/you/workspace/team-a`

Avoid:

- broad roots like `/home/you`
- system-wide paths

## Why Path Safety Matters

Agent runs can execute shell commands in configured workdirs. Broad path access
increases blast radius for accidental or unsafe commands.

## User-Level Safety Rules

1. Prefer `create_worktree` for risky tasks
2. Use clear task scopes in prompts
3. Avoid running with elevated OS privileges
4. Keep sensitive files outside `safe_paths`

## Audit-Aware Operation

For sensitive runs:

- Keep session history
- Keep prompt and output trace
- Record who triggered the task and when

This improves post-incident investigation and rollback speed.
