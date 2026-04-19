# Workspace Unified IA Spec

- Date: 2026-04-18

## Summary

Define a unified workspace-shell specification that merges Agent and Team navigation at the
product-shell level without collapsing their domain contracts.

## Decisions

- Keep `team` and `agent` as distinct entity types.
- Introduce one canonical workspace shell with shared global lenses:
  - `Chat`
  - `Threads`
  - `Tasks`
  - `Members`
  - `Search`
- Use one shared entity directory grouped as:
  - `Channels`
  - `Teams`
  - `Agents`
- Preserve Team-specific primary tabs:
  - `Chat`
  - `Kanban`
  - `Execution Runs`
  - `Members`
- Promote Agent to a first-class object with primary tabs:
  - `Chat`
  - `Tasks`
  - `Workspace`
  - `Profile`
  - `Activity`
- Keep the existing Notion-style compact, content-first visual direction.

## Follow-Up

- Converge the top-level route shell before rewriting Team or Agent inner panes.
- Add shared frontend entity/lens view models and route tests.
