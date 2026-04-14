# Worker Action-First Contract Tightening

## Why

Worker sessions were too willing to acknowledge a new concrete assignment with a clean status
summary and an intention statement ("I will investigate next") without performing the first actual
inspection step in the same turn. That behavior is safe but too passive: it preserves clarity while
failing to advance execution.

## What changed

- Tightened `skills/team/team-worker-executor.SKILL.md`.
- Added an explicit rule that a concrete assigned task with no blocker must not stop at intent
  narration.
- Added a same-turn execution rule: if the worker describes the next step, it must execute that
  concrete step in the same turn unless blocked by missing permissions, missing inputs, or runtime
  failure.
- Added a first-turn contract for new assignments:
  - acceptable first-turn artifacts include issue/PR/file inspection, code search, or a focused
    reproduction command;
  - pure "task received / scope confirmed / I will ..." summaries do not count as execution.

## Intended effect

- Shift worker behavior from "clear planning statement first" to "minimal action plus concise
  status".
- Keep the reporting discipline, but make reporting subordinate to immediate execution when the
  task is already actionable.
- Reduce cases where leader/human sees progress language without any new evidence artifact.
