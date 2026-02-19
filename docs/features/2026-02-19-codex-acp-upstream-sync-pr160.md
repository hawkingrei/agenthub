# Codex ACP Upstream Sync: PR160 Approval ID and Model API Alignment

## Summary

Migrate `zed-industries/codex-acp#160` into `agenthub-codex-acp` so local ACP
adapter behavior matches upstream for approval IDs, model-manager API changes,
and model reroute event handling.

## Upstream References

- PR: `https://github.com/zed-industries/codex-acp/pull/160`
- Commit `3d829c98ec5380e1c0a46900bac3268caf014c03`
  - `Update to latest codex main (1946a4c)`
- Commit `008e27f52c43fca32ac910729f78511db1b861b4`
  - `Update call ids`
- Commit `1188cad9ef884f7abcd45311e7e1ed04eddaf735`
  - `Rollback native-tls version`

## Background

Local `agenthub-codex-acp` still used older codex APIs:

1. Approval submit path reused prompt `submission_id` for patch/exec approvals.
2. `ModelsManager` integration still passed `Config` into
   `get_default_model/list_models`.
3. `EventMsg::ModelReroute` was not consumed.

After upstream codex update (`c34b30a`), these mismatches can cause wrong
approval binding and API drift.

## Scope

- `agenthub-codex-acp/src/thread.rs`
- `Cargo.lock`
- `docs/todo.md`

## Key Decisions

1. Keep approval binding at event granularity:
   - `Op::PatchApproval.id = call_id`
   - `Op::ExecApproval.id = approval_id.unwrap_or(call_id)`
   - `Op::ExecApproval.turn_id = Some(turn_id)`
2. Remove prompt-state `submission_id` field to avoid stale/global approval ID
   coupling.
3. Align model-manager adapter signatures with updated codex core:
   - `get_model(&Option<String>)`
   - `list_models()`
4. Handle `EventMsg::ModelReroute` as structured log output to preserve
   observability when codex reroutes model usage.
5. Update workspace lockfile to codex git revision `c34b30a` to keep adapter
   code and protocol/core APIs in sync.

## Validation

```bash
cargo check -p agenthub-codex-acp
```

Expected outcomes:

- `agenthub-codex-acp` compiles against codex `c34b30a` APIs.
- Patch/exec approval submit paths use event-level IDs instead of prompt
  submission ID.
- Model/config APIs continue to return valid presets/current model values after
  signature migration.
- `ModelReroute` events no longer fall into unexpected-event warnings.
