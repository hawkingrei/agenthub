## Summary

Cleaned up the remaining low-risk PR #127 review feedback and aligned the branch state with the already-landed runtime/UI behavior.

## Changes

- Fixed `upsertAgentEventList(..., "replace")` so history preservation only applies when a concrete `sessionId` is known.
- Simplified Team conversation merge reuse logic to rely on stable message identity/scalar fields instead of `JSON.stringify(...)` payload comparisons on every refresh.
- Tightened Team member profile parsing so partially numeric `agent_loop_idle_seconds` values are treated as unset instead of being silently coerced.
- Aligned Team member loop-idle draft/save validation with the backend `10..=86400` contract so out-of-range values are cleared instead of being carried through the spec/UI.
- Updated Team task-message SSE to preserve real internal failures as `500` responses instead of collapsing them into `404 team not found`.
- Removed the unnecessary `create_dir_all($HOME)` call from global `.agenthubmemory` gitignore setup.
- Narrowed `dead_code` suppression on `TeamRuntimeMemberRuntimeHint` to only the currently-unused fields.

## Validation

- `cd web && npx vitest run src/pages/team/page_helpers.test.ts src/pages/team/create_helpers.test.ts src/pages/team/use_team_conversation_effects.test.tsx`
- `cd web && npm run lint -- src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts src/pages/team/create_helpers.ts src/pages/team/create_helpers.test.ts src/pages/team/use_team_conversation_effects.ts`
- `cd web && npm run build`
- `cargo test team_task_messages_sse_preserves_internal_errors_as_500 -- --nocapture`
