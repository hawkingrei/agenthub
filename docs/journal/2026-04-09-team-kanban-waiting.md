Added `waiting` as a first-class Team task status for Kanban and actor tooling.

What changed:

- Extended the shared Team task status contract to include `waiting`.
- Updated Team task parsing, codec serialization, router validation, and CLI help text to accept `waiting`.
- Added a dedicated `Waiting` lane/filter to the Team Kanban UI.
- Clarified prompt and Team lifecycle guidance so `waiting` means a task is paused on human or external action such as PR review, approval, or upstream feedback.

Intent:

- `waiting` is distinct from `in_review`.
- `in_review` remains the handoff state after implementation evidence is ready.
- `waiting` is for tasks that should stay visible in Kanban while agents stop active execution until another party responds.
- Re-checking a waiting dependency does not by itself resume the task; if no new information appears, the task should remain `waiting`.
