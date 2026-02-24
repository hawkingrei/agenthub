# Codex ACP Upstream Sync: Thread State and Parallel Command Tracking

## Summary

Sync two upstream `zed-industries/codex-acp` thread-state commits into
`agenthub-codex-acp`:

1. `60007c7`: support tracking concurrent exec commands by `call_id`.
2. `4e42eca`: remove `TaskState` split and keep `/compact` and `/undo` on the
   same prompt state pipeline.

## Upstream References

- `60007c7`
  - "fix: don't lose active command for parallel commands"
- `4e42eca`
  - "cleanup: remove Task state for compact/undo"

## Background

The local ACP adapter used single-slot command tracking
(`active_command: Option<_>`), which can drop command state when multiple
`exec_command_begin` events overlap. It also used a separate `TaskState` flow
for `/compact` and `/undo`, duplicating event handling paths that were already
implemented by `PromptState`.

## Scope

- `agenthub-codex-acp/src/thread.rs`
- `docs/todo.md`

## Key Decisions

1. Replace single active command slot with
   `active_commands: HashMap<String, ActiveCommand>`.
2. Route `exec_approval`, `exec_command_begin`, output delta updates, terminal
   interactions, and completion through the per-`call_id` map.
3. Remove `TaskState` and always instantiate `SubmissionState::Prompt(...)` for
   prompt submissions, including `/compact` and `/undo`.
4. Add a focused unit test that emits interleaved command begin/end events and
   asserts both command tool calls complete.

## Validation

```bash
cargo test -p agenthub-codex-acp
```

Expected outcomes:

- No regressions in existing prompt, compact, undo, review, and custom prompt
  tests.
- Interleaved command events produce two distinct tool-call begin notifications
  and two matching completed updates.
- Prompt handling for `/compact` and `/undo` remains functionally correct after
  TaskState removal.
