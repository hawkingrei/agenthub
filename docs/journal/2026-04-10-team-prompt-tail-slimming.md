# Team Prompt Tail Slimming Follow-Up

## Summary

- tightened the default Team leader and worker prompts so they explicitly keep volatile runtime detail out of prompt prose
- clarified that durable coordination/task state should live in `AGENTS.md`, `TODO.md`, `.agenthubmemory/`, and `.cache/context/` instead of being replayed inline every turn
- kept the prompt-facing working set intentionally small: current objective or assignment, next action, allowed-action gate, and compact blocker notes

## What Changed

- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
  - added a leader policy rule that pushes durable coordination state into `AGENTS.md` / `TODO.md`
  - added a pointer-first reminder for overflow evidence under `.cache/context/run/<run_id>/...`
  - made the intended active prompt working set explicit
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
  - added a worker policy rule that keeps long execution detail in `.agenthubmemory/` or `.cache/context/run/<run_id>/...`
  - made pointer-first summaries the default when reporting durable detail
  - made the intended worker prompt working set explicit
- `crates/agenthub-team-prompts/src/lib.rs`
  - extended prompt-contract assertions so later edits do not silently remove the prompt-tail-slimming guidance

## Why

- `docs/todo.md` already calls out prompt-tail slimming as an active Team runtime follow-up.
- The current prompts had good role and workflow rules, but they did not say clearly enough that volatile runtime detail should move into file-backed memory/index artifacts.
- Making this explicit keeps the stable prefix useful while reducing the chance that prompts turn into replay buffers for task history, mailbox chatter, or large tool outputs.

## Validation

- `cargo test -p agenthub-team-prompts`
