# Actor Send File-First Inputs

## Summary

- extended `agenthub actor send` with `--text-file` and `--payload-file`
- updated Team mailbox/leader/worker skill examples to prefer file-backed sends
- documented the file-first rule so agent-facing examples keep `agenthub` at the command prefix

## Why

- Team agents were still imitating heredoc-and-shell-variable examples for multi-line mailbox sends
- those examples weaken runtime permission checks because the command no longer starts with
  `agenthub` and often relies on shell substitution
- file-backed sends keep the mailbox content reviewable/reusable while preserving a stable
  `agenthub actor ...` prefix for allow-rule matching

## What Changed

- CLI parsing:
  - `agenthub actor send` now accepts `--text-file <path>` as an alternative to `--text`
  - `agenthub actor send` now accepts `--payload-file <path>` as an alternative to
    `--payload-json`
  - `--text` conflicts with `--text-file`
  - `--payload-json` conflicts with `--payload-file`
- CLI help:
  - quick-start examples now point to `--text-file`
  - send-topic help now explains when to use `--text`, `--text-file`, `--payload-json`, and
    `--payload-file`
- Team skills:
  - shared `skills/team/AGENTS.md` now records the file-first mailbox rule
  - `team-actor-mailbox`, `team-worker-executor`, and `team-leader-orchestrator` now show
    `agenthub actor send --text-file ...` / `--payload-file ...` instead of heredoc patterns

## Validation

- `cargo test -p agenthub actor_cli::tests::parse_send_accepts_text_and_preserves_markdown -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::parse_send_accepts_text_file_and_preserves_markdown -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::parse_send_rejects_text_and_text_file_together -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::parse_send_payload_json_marks_payload_source_for_warning -- --nocapture`
- `cargo test -p agenthub actor_cli::tests::parse_send_payload_file_marks_payload_source_for_warning -- --nocapture`
- `cargo fmt --all --check`
