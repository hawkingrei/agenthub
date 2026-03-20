# Team channels deliver messages without an active run

## Summary

`Teams -> all` previously depended on a Team `active run` before human messages could reach agent
mailboxes. That broke the intended Team model:

- channels are the communication and review surface;
- Kanban remains the task surface;
- agents should still receive shared-thread messages while the Team runtime is online, even when no
  execution run is active.

This change makes shared-thread channel delivery Team-scoped instead of active-run-scoped.

## What Changed

- `src/api/teams.rs`
  - `send_team_task_message()` now forwards shared-thread human chat even when there is no active
    Team run.
  - mailbox forwarding resolves a usable run in this order:
    - current active Team run, when one exists;
    - otherwise, for the shared `all` thread only, an internal shared-thread mailbox run created on
      demand.
  - mailbox type-hint wakeups no longer suppress repeated `chat_message` deliveries just because an
    earlier chat message for the same actor is still pending; only non-chat payload types continue to
    use same-type suppression.
- `src/team/manager.rs`
  - added `ensure_shared_thread_mailbox_run()` to create or reuse an internal mailbox-only run for
    the Team shared thread;
  - hid those internal mailbox runs from normal Team run listings so they do not leak into the
    user-facing `Runs` view or distort Kanban/task semantics.
- `web/src/pages/team_page.tsx`
  - when the selected conversation is the shared thread, the page now loads the task detail and,
    if needed, the mailbox snapshot from the shared-thread mailbox run;
  - `Seen by` now merges delivered mailbox messages from both the visible snapshot and the
    shared-thread mailbox source.
  - follow-up: when there is no visible active run, `Seen by` now derives member IDs from the Team
    runtime roster instead of the active-run snapshot, and the shared-thread composer refreshes the
    hidden mailbox snapshot immediately after each send so delivered counts do not stay stale at
    `0` until a manual reload.
  - follow-up fix: the shared-thread post-send refresh helper now tolerates `null` active-run ids
    from the Team workspace state, so `NO ACTIVE RUN` sends refresh the hidden mailbox path instead
    of throwing on `.trim()`.
  - follow-up fix: `Teams -> all` now polls the selected shared thread while the `Conversation`
    workspace stays active, so agent replies and delivered `Seen by` counts continue updating
    without requiring a manual refresh or a self-send trigger.
  - the Team primary workspace no longer treats `Mailbox` as a first-class planning surface; the
    run-scoped mailbox view now lives under `Advanced` as `Execution Mailbox`, while the main Team
    workspace stays focused on `all`, `Kanban`, and `Runs`.
- Tests
  - added API coverage for shared-thread message forwarding without an active run, including inbox,
    ack, and agent reply persistence;
  - extended Team manager coverage so hidden shared-thread mailbox runs stay out of `list_runs()`
    while remaining discoverable through `get_latest_run_for_task()`;
  - added a frontend helper regression for merged mailbox sources in the shared thread.

## Why this fixes the issue

Before this change:

- the Team could be `running` with online agents;
- `all` messages were stored in the conversation thread;
- mailbox fan-out aborted immediately when no active run existed;
- `Seen by` stayed at `0`, and no agent could reply.

After this change:

- shared-thread channel messages always get a Team-scoped mailbox delivery path;
- agents can ack and reply from that path even when the Team has no active execution run;
- the shared thread can surface mailbox delivery state again through `Seen by`;
- the `all` channel keeps refreshing while open, so new agent replies do not stay hidden until a
  manual reload.

## Validation

- `cargo test team_task_messages_api_forwards_shared_thread_human_chat_without_active_run`
- `cargo test team_run_messages_emit_mailbox_type_hints_once_per_pending_payload_type`
- `cargo test list_runs_supports_status_filter_and_cursor`
- `cargo test teams_router_http_contract`
- `cd web && npx vitest run src/pages/team/page_helpers.test.ts`
- `cd web && npx vitest run src/pages/team/forge_helpers.test.ts src/pages/team/page_helpers.test.ts`
- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npx vitest run src/pages/team/use_team_conversation_effects.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team/page_helpers.test.ts`
- `cd web && npm run build`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`

## Chrome DevTools MCP

Live baseline was captured on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
before deployment of this change:

- Team header showed `TEAM RUNNING · NO ACTIVE RUN · 3 MEMBERS · 3 ONLINE`;
- messages in `all` were stored successfully;
- each message still rendered `Seen by 0 agents`;
- expanded message details showed only conversation metadata (`route = group_chat`, `to = -`),
  confirming there was no mailbox delivery path.

Post-edit Chrome MCP regression on the deployed domain is still pending until this change is rolled
out.
