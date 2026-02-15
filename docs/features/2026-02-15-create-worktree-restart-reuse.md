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
4. Add a router-level regression test to cover repeated
   `start -> stop -> start` for a `create_worktree` agent.

## Validation

```bash
cargo test create_worktree_agent_can_start_again_after_stop -- --nocapture
cargo test worktree_list_contains_path_ -- --nocapture
cargo test start_route_with_actor_runtime_payload_injects_actor_envs -- --nocapture
cargo test --all
```
