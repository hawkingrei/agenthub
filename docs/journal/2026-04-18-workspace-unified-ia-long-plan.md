# Workspace Unified IA Long Plan

- Date: 2026-04-18

## Summary

Complete the unified workspace information-architecture spec as a staged rollout plan instead of a
single large UI rewrite.

## Staged Plan

1. Phase 1: shell convergence
   - canonical `Workspace` shell language
   - shared route selection
   - backward-compatible route aliases
   - no inner Team/Agent pane rewrites
2. Phase 2: global lens convergence
   - shared `Chat` / `Threads` / `Tasks` / `Members` / `Search`
   - stable separation between shell lenses and entity-local tabs
3. Phase 3: entity promotion and shared rail
   - Agent as a first-class workspace object
   - shared `Channels` / `Teams` / `Agents` rail
   - compact-layout parity

## Guardrails

- preserve Team task-first semantics
- preserve Agent ACP/runtime/workspace depth
- preserve the current Notion-style content-first shell
- avoid backend contract rewrites during shell-convergence phases
