## Summary

- documented a local Codex allow-rule for repeated `agenthub actor ...` mailbox commands
- standardized agent-facing actor CLI examples around the shorter `agenthub actor ...` form
- recorded follow-up review fixes for actor help parsing and permission-review validation

## Why

- Team runtime mailbox coordination is now CLI-first, so local Codex approval prompts should not
  interrupt repeated `agenthub actor inbox/ack/send` usage
- runtime prompts and skills should converge on a single canonical actor CLI form to reduce token
  waste and avoid mixed examples
- the follow-up review fixes changed user-visible parsing and server-side validation behavior and
  should be tracked with the same operator-facing note

## What Changed

- appended `prefix_rule(pattern=["agenthub", "actor"], decision="allow")` guidance for
  `~/.codex/rules/default.rules`
- documented the short actor CLI workflow:
  - `agenthub actor inbox`
  - `agenthub actor ack --message-id <id>`
  - `agenthub actor send --to-actor-id <actor_id> --text "<markdown>"`
- noted review follow-ups:
  - literal `help` is only treated as the explicit help subcommand position
  - generic flag parsing only recognizes `--help` / `-h`
  - worker execution examples now use `agenthub actor ack/send ...` consistently
  - internal permission-review control rejects requests that set both `option_id` and `outcome`

## Validation

- manually reviewed:
  - `userdocs/docs/advanced/team-workbench.md`
  - `skills/team/team-worker-executor.SKILL.md`
  - `crates/agenthub-acp/src/actor_runtime_skill.rs`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub internal_grpc_permission_review_respond_rejects_conflicting_outcome_fields -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
