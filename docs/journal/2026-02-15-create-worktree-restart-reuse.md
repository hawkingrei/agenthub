# Create Worktree Restart Reuse

## Summary

Fix `create_worktree` start behavior so an existing non-empty workdir is
reused when it is already a valid git worktree of the configured repository.

## Background

`prepare_worktree_with_paths` previously rejected any non-empty target workdir
for `create_worktree`. After process restart, existing agents still point to
their already-created worktree paths, so the next start failed with
`workdir is not empty`.

## Scope

- `src/agent/manager/runtime.rs`
- `src/api/agents.rs`
- `docs/todo.md`

## Key Decisions

1. Keep strict rejection for unknown non-empty directories in
   `create_worktree` mode to avoid writing into arbitrary paths.
2. Before rejecting non-empty workdir, query
   `git worktree list --porcelain` on the configured repo.
3. If the target workdir is already present in that worktree list, treat it as
   a valid reuse path and continue start flow.
4. Add router-level regression tests to cover repeated
   `start -> stop -> start` and `state rebuild -> start` for
   a `create_worktree` agent.
5. Harden reuse safety: reject reuse when the same workdir is already bound to
   another agent record.
6. Validate existing worktree ref against configured `worktree_ref` (except
   `HEAD` which intentionally allows current head semantics).
7. Serialize reuse audit details as JSON text to avoid log field injection from
   untrusted path strings.

## Validation

```bash
cargo test create_worktree_agent_can_start_again_after_stop -- --nocapture
cargo test create_worktree_agent_can_start_after_state_rebuild -- --nocapture
cargo test create_worktree_rejects_reuse_by_other_agent -- --nocapture
cargo test parse_worktree_list_extracts_entries -- --nocapture
cargo test worktree_ref_matches_ -- --nocapture
cargo test start_route_with_actor_runtime_payload_injects_actor_envs -- --nocapture
cargo test --all
```
