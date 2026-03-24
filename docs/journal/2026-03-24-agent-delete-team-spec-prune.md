# Agent Delete Team Spec Prune

## Summary

- Fixed `DELETE /api/agents/:id` so removing an agent also prunes matching Team `spec.members[]` entries and their dependent `spec.steps[]` references.
- Kept `DELETE /api/agents/:id` successful even if the Team-spec prune follow-up later fails, so callers do not see a false-negative delete after the agent row is already gone.
- Narrowed Team pruning to only Team definitions that actually reference the deleted `member_id` instead of scanning every Team on each delete.
- Preserved valid surviving Team steps when possible and regenerated the default Team workflow only when the deleted member invalidated the current leader or step graph.
- Tightened the Team page fallback backfill hook so cached hidden agents are revalidated and cleared to `null` after external deletion instead of sticking forever in client memory.

## Why

- Agent deletion already removed the `agents` row and runtime sidecars, but Team definitions could keep referencing the deleted `member_id`.
- The Team page also cached hidden Team member agents in `teamMemberAgentsById` and skipped revalidation once an entry existed, so external deletions could leave a stale agent name visible until a hard reload.
- Together these gaps made a deleted Team member look half-deleted: backend data and UI state diverged.

## Validation

- Targeted Rust regression test covers deleting an agent that is still referenced by a Team spec and asserts the Team definition is pruned.
- Targeted Rust regression test covers the best-effort delete path when Team prune follow-up fails after the agent row is already deleted.
- Targeted web hook regression test covers an already-cached hidden Team member agent that now returns `404` and asserts the fallback cache is cleared to `null`.

## Follow-up

- Verify the deployed Team workbench path end to end: deleting a Team member agent should immediately remove the member from the active Team spec when appropriate, and stale agent names should not reappear after background refresh.
