# Team Channel/Thread Rollout Closure

## Summary

- Closed the Team `channel + thread` rollout against the current implementation surface.
- Confirmed `# all` remains the default lane while non-default channels are API-backed, sidebar-visible, routable lanes.
- Confirmed thread state is rooted in existing channel messages through stable `channel` / `thread` deep links, the right-side thread pane, and actor/API thread capabilities.
- Confirmed prompt/runtime guidance now encodes `channel root = summary entrypoint` and `thread = full context container`.

## Background

[todo.md](../todo.md) still described the channel/thread rollout as incomplete even though the implementation had already landed across backend, internal control, actor CLI, web routing, split-pane UI, prompt defaults, and E2E fixtures. This note records the post-merge closure evidence so the active backlog does not keep stale P0+/P0 entries.

## Scope

- Team channel directory and non-default channel behavior.
- Channel/thread route and deep-link behavior.
- Right-side `TeamThreadPane` split-view behavior.
- Thread reply API and actor capability integration.
- Coordinator/worker prompt guidance for summary-first channel roots and full-context threads.
- Focused tests and browser-level E2E coverage already present in the tree.

## Key Decisions

- Treat the first Team channel/thread rollout as closed when the core communication contract is implemented and covered: default channel, descriptive non-default channels, stable deep links, right-side thread pane, channel-rooted replies, actor/API capabilities, and prompt guidance.
- Keep `thread` subordinate to `channel`; no top-level workspace thread lens is introduced.
- Keep canonical task/run ownership separate from channel/thread communication. Kanban and Execution Runs remain the canonical task/run surfaces.
- Keep the composer task-draft affordance outside this closure. It remains a later product slice because it changes task-intent semantics, not the channel/thread communication foundation.

## Validation

Existing focused coverage supporting this closure:

```bash
cargo test -p agenthub-team-prompts
cargo test teams_api_injects_role_workflow_prompt_policy_defaults -- --nocapture
cargo test actor_help_for_team_thread_topics_describes_summary_first_flow_and_participants -- --nocapture
cargo test team_thread_reply_api_appends_reply_metadata_for_root_message -- --nocapture
cargo test team_thread_reply_api_notifies_existing_thread_participants -- --nocapture
cargo test create_team_channel_creates_bootstrap_conversation_and_hides_it_from_task_list -- --nocapture
cargo test delete_team_channel_cleans_bootstrap_rows_and_rejects_all -- --nocapture
cargo test internal_grpc_team_channel_and_thread_controls_round_trip -- --nocapture
cd web && pnpm exec vitest run src/api.test.ts src/pages/team_page.helpers.test.ts src/pages/team/page_helpers.test.ts src/pages/team/team_workbench_content.test.tsx src/pages/team/team_thread_pane.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx
cd web && pnpm exec playwright test tests/e2e/team_page_channels.e2e.ts --project chromium
```

Implementation evidence already present:

- `src/team/manager.rs` owns Team channel create/delete, `open_thread`, and `reply_thread`.
- `src/api/teams.rs` exposes public channel list/create/delete and thread reply endpoints.
- `src/internal/service/rpc.rs` exposes internal channel/thread controls for actor-side usage.
- `src/actor_cli.rs`, `src/actor_cli/parse.rs`, `src/actor_cli/execute.rs`, and `src/actor_cli/help.rs` expose `team-thread-open` and `team-thread-reply`.
- `web/src/pages/team_page.tsx` binds `channel` / `thread` route state to the selected channel timeline and optional right-side thread pane.
- `web/src/pages/team/team_thread_pane.tsx` renders the focused thread context and summary-first helper copy.
- `web/tests/e2e/team_page_channels.e2e.ts` covers default channel rendering, custom channel create/switch/delete, and direct channel/thread navigation.

## Follow-Ups

- Keep deployed-site validation separate from this rollout closure; [todo.md](../todo.md) still tracks deployment verification surfaces that require live `agenthub.hawkingrei.com` evidence.
- Treat composer task-draft affordances as a later product slice because they affect task materialization semantics.
