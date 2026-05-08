# Agent Trace Diagnostics

## Summary

Added the first debug-build-only `agenthub doctor agent-trace` diagnostic slice for stuck AgentHub
agents and Team member ACP no-output investigations.

## Background

Agent Team ACP turns can appear stuck after a user sends input to a member agent. Before using the
browser as the primary evidence source, operators need a backend-first way to classify whether the
stall is in runtime ownership, persisted ACP events, permission review, mailbox delivery, or a later
SSE/frontend boundary.

## Scope

- Added a project skill, `agenthub-agent-debug`, that routes stuck-agent investigations through a
  backend trace first and only moves to Chrome DevTools MCP after persistence and SSE are proven
  healthy.
- Added the debug-only `agenthub doctor agent-trace` CLI surface for standalone agents and
  Team-scoped members.
- Added read-only SQLite snapshot diagnostics for agent rows, active sessions, per-agent ACP event
  cursors, pending permission requests, pending Team mailbox messages, and likely stall verdicts.
- Added human-readable and `--json` output.
- Hard-disabled the diagnostic command in release builds.
- Kept diagnostic output redacted by construction: IDs, timestamps, statuses, event types, and
  counts are allowed; prompt bodies, message bodies, tool arguments, tool output bodies, environment
  values, and provider tokens are not emitted.

## Key Decisions

- The first slice intentionally uses read-only SQLite snapshots so it is safe for local diagnosis
  and AgentHub-managed agents.
- Provider-adapter live progress and SSE broadcaster freshness are not fabricated from the database
  snapshot. They are reported as unavailable until a live backend diagnostic path can read in-memory
  runtime/SSE state.
- The workflow remains backend-first: browser inspection is downstream evidence, not the first
  step, unless the backend trace already proves persistence and emission are healthy.

## Validation

```bash
python3 /Users/weizhenwang/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/agenthub-agent-debug
cargo fmt --all --check
cargo test -p agenthub diagnostics::agent_trace
cargo test -p agenthub doctor_cli
cargo check -p agenthub
cargo clippy -p agenthub --all-targets -- -D warnings
gh pr checks 559 | cat
```

PR #559 also resolved the review feedback for read-only SQLite busy timeout, blank `agent_id`
normalization, unnecessary Team member clone, and UUID-backed temporary test directories.

## Follow-Ups

- Extend `agenthub doctor agent-trace` to call a live backend diagnostic path when the server is
  available so it can include provider-adapter progress and SSE broadcaster freshness instead of
  database-only placeholders.
- After the live backend path lands, run Chrome DevTools MCP only for cases where persisted ACP
  events and SSE emission are both healthy but the Team ACP UI still renders stale state.
