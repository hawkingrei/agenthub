# Team Channel And Thread Backend Primitives

## Summary

This change adds the first backend-only Team channel/thread primitives needed by the
`team-channels-threads` specification:

- internal gRPC RPCs for `CreateTeamChannel`, `DeleteTeamChannel`, and `OpenTeamThread`
- actor CLI surface for `team-thread-open`
- Team manager support for:
  - creating non-default Team channels as bootstrap `team_tasks` + `team_conversations`
  - deleting non-default channels while preserving the reserved `# all` lane
  - opening a thread rooted in an existing channel message

## Contracts

- `# all` remains the reserved default channel and cannot be deleted.
- Non-default channels materialize as hidden bootstrap task/conversation rows with
  `bootstrap_kind = "team_channel"`.
- Hidden channel bootstrap tasks must stay out of normal Team task listings.
- `OpenTeamThread` is channel-first:
  - it accepts `team_id`, `channel_id`, and `root_message_id`
  - it resolves the canonical shared lane when `channel_id = "all"`
  - it rejects missing or detached root messages
- Thread identity is currently `root_message_id`-backed (`thread_id = root_message_id.to_string()`).

## Validation

- `cargo test create_team_channel_creates_bootstrap_conversation_and_hides_it_from_task_list -- --nocapture`
- `cargo test delete_team_channel_cleans_bootstrap_rows_and_rejects_all -- --nocapture`
- `cargo test open_team_thread_supports_shared_and_custom_channels -- --nocapture`
- `cargo test parse_team_thread_open_defaults_to_shared_channel -- --nocapture`
- `cargo test parse_team_thread_open_rejects_non_positive_root_message_id -- --nocapture`

## Follow-Up

- Wire the Team shell to these internal RPCs so `Channels` can create/delete real lanes.
- Add `team_thread_reply` so thread panes can move from shell-only state to actor-backed replies.
