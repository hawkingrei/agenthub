# Team Channel And Thread Backend Primitives

## Summary

This change adds the first backend-only Team channel/thread primitives needed by the
`team-channels-threads` specification:

- internal gRPC RPCs for `CreateTeamChannel`, `DeleteTeamChannel`, and `OpenTeamThread`
- internal gRPC RPC `ReplyTeamThread`
- actor CLI surface for `team-thread-open` and `team-thread-reply`
- public Team HTTP thread-reply path for the web shell:
  - `POST /api/teams/:team_id/channels/:channel_id/threads/:root_message_id/replies`
- Team manager support for:
  - creating non-default Team channels as bootstrap `team_tasks` + `team_conversations`
  - deleting non-default channels while preserving the reserved `# all` lane
  - opening a thread rooted in an existing channel message
  - replying to that thread without inventing a detached thread table first
  - letting the Team thread pane render replies while keeping thread replies out of the main
    channel timeline

## Contracts

- `# all` remains the reserved default channel and cannot be deleted.
- Non-default channels materialize as hidden bootstrap task/conversation rows with
  `bootstrap_kind = "team_channel"`.
- Hidden channel bootstrap tasks must stay out of normal Team task listings.
- `OpenTeamThread` is channel-first:
  - it accepts `team_id`, `channel_id`, and `root_message_id`
  - it resolves the canonical shared lane when `channel_id = "all"`
  - it rejects missing or detached root messages
- `ReplyTeamThread` is also channel-first:
  - it accepts `team_id`, `channel_id`, `root_message_id`, and reply `text`
  - it validates the root message through the same channel resolver as `OpenTeamThread`
  - it appends a `team_thread_reply` conversation message carrying
    `payload.thread_root_message_id = root_message_id`
- Team web shell projection:
  - the center channel timeline hides `team_thread_reply` rows
  - the right-side thread pane filters replies by `thread_root_message_id`
  - thread replies submit through the public HTTP path instead of depending on internal gRPC
- Thread identity is currently `root_message_id`-backed (`thread_id = root_message_id.to_string()`).

## Validation

- `cargo test create_team_channel_creates_bootstrap_conversation_and_hides_it_from_task_list -- --nocapture`
- `cargo test delete_team_channel_cleans_bootstrap_rows_and_rejects_all -- --nocapture`
- `cargo test open_team_thread_supports_shared_and_custom_channels -- --nocapture`
- `cargo test parse_team_thread_open_defaults_to_shared_channel -- --nocapture`
- `cargo test parse_team_thread_open_rejects_non_positive_root_message_id -- --nocapture`
- `cargo test parse_team_thread_reply_defaults_to_shared_channel -- --nocapture`
- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`

## Follow-Up

- Wire the Team shell to these internal RPCs so `Channels` can create/delete real lanes.
- Expand the current thread-pane reply wiring into a fuller `channel + thread` rollout:
  reply counts on root messages, stronger browser-level regression coverage, and stable deep-link
  recovery still remain.
