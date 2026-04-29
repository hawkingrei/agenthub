# Team Channel API Rollout Checkpoint

## Scope

This checkpoint advances the top Team workspace TODO item by wiring the first public Team channel
API slice and projecting it into the Team shell.

Implemented in this pass:

- add public Team channel HTTP routes:
  - `GET /api/teams/:team_id/channels`
  - `POST /api/teams/:team_id/channels`
  - `DELETE /api/teams/:team_id/channels/:channel_id`
- keep `ReplyTeamThread` on the existing stable public route
- expose `TeamChannelRecord` and channel CRUD helpers in the web API client
- replace the frontend-only `"all"` channel type restriction with a real string channel id model
- fetch non-default channels in `TeamPage`
- map `?channel=:channel_id` to the corresponding hidden bootstrap task conversation so the Team
  center pane can switch between `# all` and non-default channels
- preserve `# all` as the default lane and fall back to it when the requested channel no longer
  exists
- add Team shell create/delete channel controls in the sidebar, keep `# all` reserved, switch to
  the newly created channel automatically, and fall back to `# all` after deleting the active
  non-default channel
- treat non-default channel bootstrap conversations as first-class channel lanes for the Team
  composer and thread pane, while keeping arbitrary task conversations outside the channel-thread
  flow
- rename the internal conversation flag from `isSharedConversation` to `isChannelConversation`
  so the Team shell no longer conflates `# all` with other channel lanes
- encode explicit Team task conversations into canonical Team workspace routes via `task=<task_id>`
  so Kanban / task-surface entry points can restore the selected conversation on refresh instead of
  silently falling back to the active channel bootstrap lane

## Validation

Rust:

- `cargo fmt --all`
- `cargo test list_team_channels_returns_non_default_channels_in_creation_order -- --nocapture`
- `cargo test team_channel_api_lists_creates_and_deletes_non_default_channels -- --nocapture`
- `cargo test team_channel_api_maps_duplicates_and_missing_channel_errors -- --nocapture`

Web:

- `cd web && pnpm exec vitest run src/api.test.ts src/pages/team_page.helpers.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_page.agent_loop.test.tsx`
- `cd web && pnpm exec vitest run src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_thread_pane.test.tsx src/pages/team_page.agent_loop.test.tsx`
- `cd web && npm run build`
- `cd web && pnpm exec vitest run src/pages/team_page.helpers.test.ts src/pages/team_page.smoke.test.tsx`

## Remaining Rollout Gaps

The top TODO item is not closed by this checkpoint yet.

Still remaining:

- add Chrome DevTools MCP baseline + regression validation for the new multi-channel Team flow
- continue the `channel + thread` rollout so all Team surfaces (`View in channel`, channel-scoped
  task entry points, and refresh/deep-link restore with a live thread route) behave consistently
  across both `# all` and non-default channels
- close the remaining unified workspace shell follow-ups after the Team channel model is no longer
  hardcoded

## 2026-04-28 Follow-up

- converged Team workspace route construction back onto the canonical shared helper in
  `web/src/app_route_selection.ts` so `task=` deep links no longer depend on a drifting
  `team_page.tsx`-local path builder
- kept the existing `TeamPage` helper exports stable for focused tests, but the local Team route
  helper now delegates to the shared route contract instead of re-encoding its own query shape

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/app_route_selection.test.ts src/pages/team_page.helpers.test.ts`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run build`

## 2026-04-28 Follow-up 2

- extended channel-lane thread routing so explicit task conversations that still belong to the
  active non-default channel now keep `task=<task_id>` when opening a thread and when returning
  from the right-side thread pane
- this closes the route-shape gap for:
  - `?lens=channels&channel=review&task=task-work&thread=<message_id>`
  - `?channel=review&task=task-work&thread=<message_id>`
- tightened the Team task-detail CTA language so Kanban no longer labels task-conversation entry
  as `Open thread`; the task detail now says `Open conversation`, leaving `Thread` reserved for
  message-rooted split-view behavior

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/page_helpers.test.ts src/pages/team_page.smoke.test.tsx`
- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- `cd web && npm run build`
