# Team Prompt Tail Slimming

## Summary

Default coordinator and worker prompts now keep runtime recovery tails shorter and more
pointer-first, while preserving the mailbox, task, thread, and visible-reply gates that must stay
in the active role charter.

## Background

Earlier prompt-tail work moved bulky continuity detail into `.cache/context/state.md` and
run-scoped artifacts. The default Team role prompts still carried duplicated cold-start and
workflow procedure text that belongs in `AGENTS.md` plus role skills rather than in every runtime
prompt.

## Scope

- Replaced the coordinator cold-start and workflow tail with a compact runtime recovery tail.
- Replaced the worker cold-start and workflow tail with a compact runtime recovery tail.
- Kept prompt text focused on current objective, next action, allowed-action gate, blocker state,
  and the canonical mailbox/task/thread contracts.
- Added prompt-template line-count coverage so future changes do not accidentally grow the runtime
  tails again.

## Key Decisions

- The default prompt remains the role charter and allowed-action gate, not a full operating manual.
- Detailed coordinator and worker procedures stay in role skills, with `AGENTS.md` as the local
  index and `.cache/context/state.md` / `.cache/context/run/<run_id>/...` as recovery pointers.
- This slice does not close the broader P1 prompt-tail backlog. Follow-up work should still verify
  mailbox/task routing behavior under real Team runs after additional shrinkage.

## Validation

```bash
cargo test -p agenthub-team-prompts -- --nocapture
```

## Follow-Ups

- Continue evaluating whether runtime-injected state can be reduced to objective, next action,
  allowed-action gate, and compact blocker context during real coordinator/worker runs.
