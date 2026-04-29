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

## 2026-04-29 Follow-up

- tightened the Kanban/task-surface conversation entry so channel-scoped tasks route back into
  their owning Team lane instead of inheriting whichever channel the operator last had selected
- `Open conversation` from Kanban task detail now preserves:
  - `?lens=channels&channel=review&task=task-work`
  - instead of incorrectly falling back to `?lens=channels&task=task-work`
- kept the change narrow by routing only the task-detail CTA through the task-owned channel id;
  the existing channel-thread and refresh-restore behavior remains unchanged

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/use_team_workspace_view_model.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- `cd web && npm run build`

Validation note:

- local `vitest` and `vite build` attempts in this pass were blocked by `ENOSPC` while Vite tried
  to write temporary files under `web/node_modules/.vite-temp`; the new regression coverage is in
  place, but the final local run needs disk space before it can complete

## 2026-04-29 Follow-up 2

- preserved explicit task conversations when switching away from `Channels` and then returning via
  the workspace lens bar
- `onSelectWorkspaceLens("channels")` now keeps the active non-shared task conversation route
  instead of collapsing back to the selected channel bootstrap lane
- this keeps `?lens=channels&channel=review&task=task-77` stable across lens hops and finishes one
  more route-parity gap in multi-channel phase 2

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/use_team_workspace_view_model.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- browser MCP baseline remains blocked in this pass because the saved `agenthub-team-ui` browser
  session is currently back on the login page
- `vitest` still shares the same local `ENOSPC` blocker until disk space is recovered

## 2026-04-29 Follow-up 3

- canonicalized legacy `?lens=channels&task=<id>` routes when the selected task actually belongs
  to a non-default Team channel
- once the task detail loads, TeamPage now upgrades that route to the canonical lane shape:
  - `?lens=channels&channel=review&task=task-work`
- preserved `thread=<message_id>` during the same canonicalization path so older task links can
  still regain full channel-thread semantics after refresh

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- local `vitest` remains blocked by the same Vite `ENOSPC` temp-file failure until disk space is
  recovered

## 2026-04-29 Follow-up 4

- tightened the channel timeline thread affordance so it now carries live reply-count context
  instead of staying a bare `Thread` button
- channel messages with existing thread replies now render:
  - `Thread · 1 reply`
  - `Thread · N replies`
- this keeps the center timeline closer to the split-view spec: thread access is visible from the
  parent channel lane without duplicating reply bodies inline

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- focused `vitest` remains blocked by the same local `ENOSPC` condition until disk space is freed

## 2026-04-29 Follow-up 5

- marked the active thread root inside the parent channel timeline when the right-side thread pane
  is open
- the center lane now keeps a lightweight focused state on the source message instead of only
  changing the thread button copy
- this makes the split-view relationship clearer: the right pane is subordinate to a visible source
  message in the parent channel, not a detached chat surface

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- local focused `vitest` still depends on recovering disk space from the current `ENOSPC`

## 2026-04-29 Follow-up 6

- added a compact source-message summary strip to the right-side thread pane header
- thread now makes its parent-context relationship explicit with:
  - `Source`
  - `From <author> · #<root_message_id>`
- this keeps the pane subordinate to the parent channel message without adding heavy debug chrome

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- local focused `vitest` is still blocked by the same `ENOSPC` condition until disk space is freed

## 2026-04-29 Follow-up 7

- expanded the thread-pane source strip into a two-line compact context block
- the first line still keeps the canonical source identity:
  - `Source`
  - `From <author> · #<root_message_id>`
- the second line now carries a one-line preview of the original root message text when chat text is
  available
- this keeps the right pane readable as a focused child context even when the operator no longer
  has the parent channel root fully in view

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

Validation note:

- local focused `vitest` remains blocked by the same `ENOSPC` condition until disk space is freed

## 2026-04-29 Follow-up 8

- separated `View in channel` from `Close thread` behavior inside split-view
- `View in channel` now closes the right-side pane and asks the center timeline to scroll the
  source message back into view
- the jump stays as local UI state instead of route state so canonical Team URLs remain:
  - `?channel=<id>`
  - `?channel=<id>&task=<id>`
- `Close thread` still just dismisses the pane without forcing a source-message jump

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 9

- tightened the right-side thread-pane hierarchy so it now reads more explicitly as:
  - `Original`
  - `Thread replies`
  - `Reply in thread`
- replaced the previous generic helper copy with clearer reply-context language:
  - `Reply stays in this thread, not the parent channel`
- this keeps the pane acting like a focused child context instead of a second full channel surface

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 10

- canonicalized the task query that rides along channel-thread routes so bootstrap lane tasks no
  longer leak into thread open/close/view-in-channel URLs as redundant `task=` params
- explicit task conversations still keep `task=<task_id>` when they differ from the selected
  channel bootstrap task, but channel bootstrap conversations now return to:
  - `?lens=channels&channel=review`
  - instead of the redundant `?lens=channels&channel=review&task=task-review`
- the route-decision logic now lives in a focused helper with an inline comment describing the
  canonicalization boundary, so later split-view work does not drift back into local ad-hoc query
  shaping

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_page.helpers.test.ts src/pages/team_page.smoke.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
