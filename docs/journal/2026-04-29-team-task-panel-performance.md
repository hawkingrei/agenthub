# Team Task Panel Performance

## 2026-04-29 Follow-up 1

- reduced repeated read-state work in `web/src/pages/team_task_panel.tsx`
- `SeenProgressState` is now precomputed once per visible activity item instead of rebuilding the
  same breakdown inside every row render
- this keeps long channel timelines from paying repeated `seenByMessageId` normalization costs
  while preserving the existing `Seen X/Y` and pending-receipt UI behavior

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 2

- moved thread-reply counting onto a standalone memo keyed by the full ordered message list
- viewport-window changes no longer rescan the entire conversation history just to re-derive the
  `Thread · N replies` affordance for visible rows
- visible row metadata now layers on top of that precomputed map, which keeps the split-view
  affordance stable while trimming another hot path in long timelines

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 3

- reduced repeated member-ACP event scanning in
  `web/src/pages/team/use_team_member_acp_view_model.ts`
- the view model now projects visible ACP events, ACP line items, and terminal-output rows from a
  single memoized event projection instead of scattering those traversals across separate memos
- this keeps the Team member ACP surface on the same performance-hardening track as the channel
  timeline without changing the visible conversation/debug behavior

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 4

- reduced repeated mailbox conversation-row work in `web/src/pages/team_mailbox_panel.tsx`
- mailbox actor rows and visible conversation rows are now projected once so actor labels, unread
  badges, payload rendering inputs, and acceptability checks are not recomputed inside every row
  render
- this keeps long mailbox conversations on the same precomputed-row path as the Team channel
  timeline and member ACP surfaces

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 5

- pushed more Team channel row display state into the existing `visibleActivityRows` projection in
  `web/src/pages/team_task_panel.tsx`
- visible rows now precompute item/content/bubble class names, thread-button labels, and
  permission-card record state instead of re-deriving those values inside every timeline row render
- this keeps the render loop closer to a pure view over projected row metadata and trims another
  layer of repeated `Map` lookups and UI-state branching

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 6

- reduced avoidable thread-transcript rebuilds in `web/src/pages/team/team_thread_pane.tsx`
- root-message and reply display rows are now memoized so editing the reply draft does not
  continuously re-derive avatar labels, timestamps, and transcript row props for the whole pane
- this keeps the split-view reply composer closer to local-state-only updates instead of
  invalidating the entire visible thread transcript on every keystroke

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/team_thread_pane.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 7

- reduced repeated preview-row work in `web/src/pages/team_member_console_panel.tsx`
- member select options, member event rows, and run preview rows are now memoized so timestamp
  formatting, actor-label resolution, and JSON payload rendering are not recomputed inside every
  render pass
- this keeps the member console aligned with the other ACP-heavy surfaces that now render from
  projected row data instead of raw event arrays

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx -t "TeamMemberConsolePanel switches preview and member-history views"`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 8

- pushed `TeamMemberConsolePanel` detail and discovery-card field rendering onto projected detail
  item arrays instead of open-coded field-by-field JSX
- this keeps the console surface closer to pure data projection across both list and detail areas,
  and removes another layer of repeated string joining and field branching from the render path

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx -t "TeamMemberConsolePanel switches preview and member-history views"`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 9

- added a lightweight tail render window to `web/src/pages/team_member_console_panel.tsx`
- loaded member histories now render only the most recent slice of events while keeping `Load Older`
  behavior intact and surfacing an explicit notice when older loaded rows are being omitted from
  the current DOM window
- this is the first structural list-size guard in the member console, beyond the row/detail
  projections from earlier follow-ups

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team_panels.test.tsx -t "TeamMemberConsolePanel"`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## 2026-04-29 Follow-up 10

- added a lightweight Team run-context SSE invalidation stream in `src/sse.rs`
- the new `/sse/teams/{team_id}/runs/{run_id}/context` route emits compact refresh hints derived
  from a server-side run-context fingerprint instead of pushing a second full snapshot contract
- `web/src/pages/team/use_team_run_lifecycle_effects.ts` now uses that stream as the primary
  refresh path for run-focused tabs and narrows fallback polling to the minimal tab-specific reads
- `conversation`, `tasks`, and ACP/member-console surfaces are intentionally excluded from this
  active-run loop because they already have their own SSE/polling contracts and were the main
  source of the observed Team page polling storm

Focused validation for this follow-up:

- `cd web && pnpm exec vitest run src/pages/team/use_team_run_lifecycle_effects.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- `cargo test team_run_context -- --nocapture`
