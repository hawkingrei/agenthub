# Worktree Default Root

## Summary

Add a unified default worktree root (`~/.agenthub/worktrees`) and expose it to
the web UI so Create Agent starts with a stable default path.

## Background

Create Agent previously required users to repeatedly enter workdir values for
worktree-based runs. Backend and frontend had no shared default root contract,
which increased friction and could cause inconsistent behavior between clients.

## Scope

- `src/config.rs`
- `src/state.rs`
- `src/api/mod.rs`
- `src/api/settings.rs`
- `src/api/agents.rs`
- `src/main.rs`
- `src/api/teams/tests.rs`
- `web/src/api.ts`
- `web/src/app.tsx`
- `web/src/components/create_agent_modal.tsx`
- `web/src/create_agent_modal.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Introduce `worktree.default_root` in config with fallback
   `~/.agenthub/worktrees`.
2. Add authenticated `GET /api/settings/defaults` so frontend reads runtime
   defaults from backend instead of hardcoding behavior.
3. Pre-fill Create Agent `workdir` with the runtime default root and restore the
   default after successful creation.
4. Keep backend safety net: when `worktree_mode=create_worktree` and `workdir`
   is blank, generate a deterministic default path under the configured root.

## Validation

```bash
cargo test default_worktree_root_ -- --nocapture
cargo test resolve_create_agent_workdir_ -- --nocapture
cd web
npm run test -- src/create_agent_modal.test.tsx
```

## Follow-ups

- Consider exposing additional runtime defaults (for example default worktree
  ref or naming strategy) in `/api/settings/defaults` to keep web behavior
  aligned with backend policy.
