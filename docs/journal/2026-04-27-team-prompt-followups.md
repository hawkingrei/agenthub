# Team Prompt Follow-Ups

## Summary

- aligned the active Codex backlog to the post-`v0.125.0` reality by removing stale TODO items that still assumed the temporary fork pin was live or that the managed-skill / PR160 verification work was unfinished
- tightened the default Team leader and worker prompts so they explicitly treat `task` as the primary ownership object, keep `run` / `step` as execution-debug artifacts, and use `attempt` for one active push that ends when the task moves to `waiting` or `in_review`

## What Changed

- `docs/todo.md`
  - replaced the stale fork-pin TODO pair with one follow-up that matches the current official `openai/codex@rust-v0.125.0` baseline
  - removed the already-completed managed-skill materialization and PR160 sync verification TODO items from the active ACP backlog
  - generalized the old `Codex 0.121` MCP parallel-tool-call TODO so it no longer hard-codes an obsolete version number
- `docs/journal/2026-04-24-codex-custom-tool-output-hotfix.md`
  - appended a follow-up that records the shift from the temporary fork pin to the official upstream baseline and explains what remains open
- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
  - added explicit vocabulary guidance for `task`, `attempt`, `run`, and `step`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
  - added the same execution-vocabulary guidance from the worker side
- `crates/agenthub-team-prompts/src/lib.rs`
  - extended prompt-contract assertions so future prompt edits cannot silently drop the new `task` / `attempt` / `run` rules

## Why

- `docs/features/team-execution-vocabulary.md` already defines the canonical `task` / `attempt` / `run` / `step` boundary, but the default Team prompts did not state that boundary plainly enough.
- That gap makes it easier for leader/worker runs to drift back into “run means everything” language even though the canonical Team flow is task-first.
- The Codex TODO surface also drifted behind reality after PR 430 and PR 433 merged; keeping stale fork-pin language in the active backlog makes the next backend slice harder to choose correctly.

## Validation

- `cargo test -p agenthub-team-prompts`
