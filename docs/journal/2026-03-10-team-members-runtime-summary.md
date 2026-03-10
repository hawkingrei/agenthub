# Team Members Runtime Summary Merge

## Summary

Expanded the existing `team_members` runtime tool into the single Team context snapshot tool instead of adding a separate `team_runtime` tool.

## Why

- Team actor sessions already depend on one mailbox tool surface (`actor_inbox` / `actor_ack` / `actor_send`).
- Adding a second Team context tool would increase deployment and prompt-surface complexity.
- Agents need one canonical query for Team runtime state, roster/card identity, and optional run overlay.

## What Changed

- `team_members` now accepts optional `team_id` and optional `run_id`.
- `team_members` returns:
  - `runtime`: team runtime summary (`status`, `online_count`, `member_count`)
  - `members`: live roster/card/session view
  - `run`: optional run overlay when `run_id` is resolved
- actor CLI `team-members` now supports either `--team-id` or `--run-id`.
- actor runtime skill and Team prompts now describe `team_members` as the single Team context snapshot tool.

## Validation

Recommended validation commands:

```bash
cargo test actor_mcp -- --nocapture
cargo test describe_team_context_merges_runtime_summary_and_optional_run_overlay -- --nocapture
cargo test parse_team_members_ -- --nocapture
cargo test -p agenthub-acp actor_runtime_skill_includes_context_and_native_tool_contract -- --nocapture
cargo test -p agenthub-team-prompts prompt_templates_keep_required_contract_lines -- --nocapture
```

## Follow-up

- Keep the tool name `team_members` for now to avoid compatibility churn.
- Revisit naming only if Team context usage expands beyond runtime/roster/run overlay.
