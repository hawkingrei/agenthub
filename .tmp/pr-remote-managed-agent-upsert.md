## Summary

- refactor remote managed agent persistence so `ensure_remote_managed_agent` reuses one shared remote-managed field projection for both `INSERT` and `UPDATE`
- preserve legacy-schema compatibility for `source` and `target_node_id`
- add focused store-level regression coverage for full-schema insert and legacy-schema update behavior

## Testing

- `cargo test -p agenthub remote_managed_upsert_ -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `git diff --check`

## Docs

- remove the completed TODO item from `docs/todo.md`
- add `docs/journal/2026-03-30-remote-managed-agent-upsert-refactor.md`
