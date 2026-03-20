## Summary

Added leader-driven Team task tools to actor MCP so canonical Kanban cards can be created and advanced from Team sessions instead of only being discussed in conversation text.

## Changes

- Added actor MCP tools:
  - `team_tasks`
  - `team_task_create`
  - `team_task_update`
- Restricted canonical task creation and lifecycle updates to leader.
- Kept `team_tasks` readable for workers so they can verify canonical Team task state without relying on mailbox payload guesses.
- Added lightweight Kanban polling in the Team page while the `Tasks` tab is active so newly created MCP tasks show up without manual refresh.

## Why

Agents were claiming that a task had been opened, but Team Kanban still showed nothing. The root causes were:

1. Team agents had no canonical task tool, so "task opened" was often only a conversational claim.
2. Even when a backend task existed, the Kanban panel did not auto-refresh while the page stayed open.

## Validation

- Rust actor MCP tests for tool exposure, leader-only task mutation, and task listing
- prompt/skill mirror tests
- web task-refresh hook tests
