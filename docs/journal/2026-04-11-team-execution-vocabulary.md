# Team Execution Vocabulary

## Summary

- added `docs/features/team-execution-vocabulary.md` as the canonical vocabulary doc for:
  - `task`
  - `attempt`
  - `run`
  - `step`
  - `round`
- clarified that `task` remains the ownership/work object, `run` remains the concrete execution
  partition, `step` stays legacy/debug-scoped, and `attempt` becomes the semantic unit for
  retry/resume accounting
- clarified that `round` is a planning/coordination cadence, not a synonym for execution `run`

## Why

The runtime and UI already distinguish task state from run telemetry, but retry, resume, and
`waiting` loops still risk collapsing back into one overloaded "run" concept.

With the workspace memory contract now documented, the next missing stable contract is the
execution vocabulary itself so prompts, docs, runtime notes, and UI follow-up work stop drifting.

## Changes

- defined explicit attempt boundaries:
  - starting active execution from `open`, `waiting`, or rework from `in_review` starts a new
    attempt
  - leaving active execution for `waiting`, `in_review`, `completed`, or `canceled` ends the
    current attempt
- documented that runtime/session replacement inside the same active execution push should not
  automatically create a new attempt
- added surface mapping guidance so:
  - `Conversation` stays intent/progress
  - `Kanban` stays task/ownership
  - `Runs` stays concrete execution partitions
  - `Steps` stays debug-oriented

## Follow-Up

- docs and prompts should adopt the new vocabulary first
- UI/runtime naming can then align incrementally without forcing one big schema rewrite
