# Team channel SSE realtime for shared-thread conversations

## Summary

`Teams -> all` already had Team-scoped mailbox delivery without requiring an active run, but the
conversation surface still depended on manual refresh or coarse polling to reveal new agent replies.
This change adds a Team conversation SSE stream so the shared-thread channel can react to new
conversation writes immediately while keeping the existing HTTP refresh path as fallback.

## What Changed

- `src/team/manager.rs`
  - added a lightweight conversation broadcaster on `TeamManager`;
  - `append_task_conversation_message()` now emits a Team conversation stream event after the
    message row is inserted.
- `src/team/manager/mailbox.rs`
  - `send_actor_message_with_created()` now emits a Team conversation stream event when a newly
    created mailbox message also persists a canonical human-visible chat reply back into the shared
    thread;
  - kept the event emission gated to newly created canonical chat replies so idempotent resend paths
    do not spuriously retrigger the channel stream.
- `src/sse.rs`
  - added `/sse/teams/:team_id/tasks/:task_id/messages?token=...`;
  - validates the session token, checks Team ownership, confirms the task/conversation exists, then
    streams matching Team conversation events plus heartbeats.
- `web/src/pages/team/use_team_conversation_effects.ts`
  - shared-thread conversations now open an `EventSource` against the Team SSE endpoint;
  - incoming Team conversation events trigger the existing `refreshTaskMessages()` path, so the
    current `Seen by` + hidden-mailbox aggregation model stays unchanged;
  - polling remains as fallback when SSE is unavailable or temporarily disconnected.
- `web/src/pages/team/use_team_conversation_effects.test.tsx`
  - added coverage for Team conversation SSE-triggered refresh and for suppressing polling while the
    SSE connection is open.

## Why this fixes the issue

Before this change, Team shared-thread replies could be delivered into the database but stay hidden
from the open `all` channel until the next manual refresh or polling tick.

After this change:

- Team conversation writes publish a lightweight stream event immediately;
- the open `all` channel refreshes itself as soon as that event arrives;
- existing HTTP refresh and hidden-mailbox snapshot aggregation continue to work as fallback and as
  the single source of truth for rendered conversation state.

## Validation

- `cargo test team_conversation_stream_emits_only_matching_events`
- `cargo test append_task_conversation_message_emits_stream_event`
- `cargo test actor_mailbox_service_persists_agent_reply_into_shared_thread`
- `cargo test team_task_messages_sse_requires_valid_token`
- `cargo test team_task_messages_sse_returns_ok_for_accessible_team_task`
- `cargo test team_task_messages_api_forwards_shared_thread_human_chat_without_active_run`
- `cargo test ensure_shared_thread_mailbox_run_is_idempotent`
- `cd web && npx vitest run src/pages/team/use_team_conversation_effects.test.tsx`
- `cd web && npm run lint -- src/pages/team/use_team_conversation_effects.ts src/pages/team/use_team_conversation_effects.test.tsx src/pages/team_page.tsx`
- `cd web && npm run build`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`

## Follow-up

- Verify the deployed Team channel now updates in real time on `agenthub.hawkingrei.com` without
  requiring manual refresh while the `all` channel stays open.
- Continue the next UI pass with Slock as a composition/interaction reference so the Team workspace
  feels less debug-heavy and more like a lightweight coordination surface.
