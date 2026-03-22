# Team Role Index Builtins

## Summary

Replaced the thin Team role index markdown files with Rust-generated builtin
skills so leader/worker index text can share one template without duplicating
nearly identical startup guidance.

## Scope

- `crates/agenthub-acp/src/team_role_skills.rs`
- `skills/team/team-leader-orchestrator.SKILL.md`
- `skills/team/team-worker-executor.SKILL.md`
- removed:
  - `skills/team/team-leader-agents-index.SKILL.md`
  - `skills/team/team-worker-agents-index.SKILL.md`

## Key Decisions

1. Keep the role index skill names stable.
   - `team-leader-agents-index`
   - `team-worker-agents-index`
   - Runtime injection contract does not change.
2. Move the shared role-index skeleton into Rust.
   - A single builder now emits the role-specific index instructions with only
     the minimum leader/worker deltas.
3. Do not fold role-specific startup text into shared `skills/team/AGENTS.md`.
   - Shared baseline should stay role-agnostic.
   - Role-only guidance should not inflate both leader and worker prompt
     surfaces.

## Why

The previous role index markdown files were very small but still duplicated the
same startup shape. Moving them into one Rust builder reduces drift and keeps
the Team skill tree focused on shared contracts plus role execution skills.

## Validation

```bash
cargo test -p agenthub-acp team_role_skills::tests::role_agents_index_builder_keeps_role_specific_contract_small -- --nocapture
cargo fmt --all --check
git -c core.fsmonitor=false diff --check
```
