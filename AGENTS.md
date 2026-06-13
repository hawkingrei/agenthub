# AgentHub Charter

This file keeps the highest-level project intent and engineering rules. Keep it short.
Detailed behavior, product flow, and execution procedures belong in `docs/features/`,
`docs/journal/`, `docs/todo.md`, or skill-specific `SKILL.md` files.

## 1. Product Goal

AgentHub is a single-binary control plane for long-lived AI agents.

- Users can create, start, stop, reconnect, and inspect agents from the web UI.
- Agents run in user-selected workspaces and must stay alive even if the browser page closes.
- Agent output and history must remain visible and replayable.

## 2. Core Architecture

- Backend: Rust, single process.
- Frontend: React + Vite SPA, served by the Rust binary.
- Database: SQLite.
- Build/test: Bazel must remain viable alongside normal Rust and web workflows.
- Agent runtime: subprocesses launched by the backend in explicit workspaces.

## 3. Engineering Principles

- Prefer small, reviewable changes.
- Keep behavior correct before making it clever.
- Add focused tests for non-trivial changes and every bugfix.
- Keep code readable and domain-oriented; split files/modules when responsibilities drift.
- Fix obvious local issues discovered during the active edit in the same change.
- Add short comments when behavior depends on non-obvious intent or compatibility boundaries.

## 4. Rust And Frontend Boundaries

- Rust code should move toward cohesive domain crates under `crates/`, not grab-bag modules.
- `cmd/agenthub/src/main.rs` should stay thin; business logic belongs in libraries.
- Frontend work should use Mantine plus Tailwind utilities by default.
- Avoid expanding legacy handcrafted global CSS except compatibility fixes.
- User-facing UI should stay clean, compact, and localization-ready.

## 5. Team And Runtime Principles

- Team conversation is user-facing; internal task planning belongs to the leader/runtime flow.
- Mailbox/actor paths are the canonical execution transport for Team coordination.
- Worker execution should stay isolated per workspace/worktree.
- Context and memory are workspace-scoped and should not leak across agents.

## 6. Documentation And Validation

- Every meaningful change should leave tracked documentation.
- Stable design belongs in `docs/features/`.
- Journal navigation belongs in `docs/journal/summary.md`.
- Implementation checkpoints belong in `docs/journal/`.
- Follow-up verification belongs in `docs/todo.md`.
- Use `.agents/skills/agenthub-docs-spec/SKILL.md` when creating or revising canonical feature specs under `docs/features/`.
- Use `.agents/skills/agenthub-docs-journal/SKILL.md` when creating or revising dated rollout notes under `docs/journal/`.
- Frontend changes should use Chrome DevTools MCP for before/after validation.
