# Summary

A code-only UI/UX review of `web/` (no running app available in the review environment) surfaced 17
findings across correctness, accessibility, and consistency, cross-checked against
[frontend-design.md](../features/frontend-design.md)'s explicit contracts. This entry covers the six
highest-priority fixes landed from that review; the remaining findings are tracked in
[todo.md](../todo.md).

# Background

The review ran four parallel code audits (conversation/composer, ACP heavy-output, accessibility/mobile
guardrails, forms/modals/loading/error/empty states), each citing file:line evidence rather than general
advice, since `frontend-design.md` is already a mature, deliberate spec -- the useful question was where
the running code actually diverges from it or has gaps it doesn't cover at all.

# Scope

1. **Error hidden behind the modal that caused it.** `CreateAgentModal` disables every dismiss
   affordance (`closeOnEscape`, `closeOnClickOutside`, close button), but submit-time validation errors
   ("workdir is required") only reached the page-level banner, which the modal visually covers. Added a
   `formError` prop rendered inside the modal itself; wired `agentsError` into it in `app.tsx`.
2. **Raw status strings leaking into user-facing UI.** `formatToolCallStatus` already existed and mapped
   `in_progress` -> "In Progress" etc., but three call sites bypassed it: `permission_modal.tsx`,
   `acp_debug.tsx`'s permission toggle, and `output_header.tsx`'s merged status label (agent lifecycle
   vocabulary, not ACP tool-call vocabulary, so given its own light capitalization helper instead of
   reusing `formatToolCallStatus`).
3. **Custom Team dialogs bypassed Mantine's built-in focus management.** `TeamCreateDialog`,
   `TeamEditMemberDialog`, and `TeamCopyExistingAgentDialog` in `team_management_modals.tsx` are
   hand-rolled `<div role="dialog">` elements with no focus trap and no focus-return-to-trigger, unlike
   every Mantine-`Modal`-based dialog in the app. Wired `@mantine/hooks`' `useFocusTrap`/`useFocusReturn`
   -- already-tested library primitives, not reimplemented logic -- into all three.
4. **Destructive actions with zero confirmation.** Only "Delete Team" and "delete channel" had a
   `window.confirm` guard; "Delete Agent" (Team workspace), node removal, "Revoke Device", "Delete Safe
   Path", and admin's "Delete Selected" (safe paths) fired on a single click. Added the same
   `window.confirm` pattern to all five.
5. **Empty admin lists rendered nothing.** Safe Paths, Devices, and Login Audits in
   `admin_page_sections.tsx` rendered a bare empty `<ul>` when empty, unlike every other list surface in
   the app, which uses the shared `EmptyState` primitive. Added it to all three.
6. **No loading-state guard on the join/register flow.** `join_page.tsx`'s three submit buttons (Join
   Device, Join Teamspace, Join as `<user>`) had no busy state, so a double-click could fire a duplicate
   join/register/accept-invite request. Added a `busy` state gate with re-entry guards, busy button text,
   and `disabled`. Also fixed an adjacent inconsistency found while in the file: the missing-token case
   rendered a raw `<div className="error">` instead of the `ErrorBanner` used three other times in the
   same file.

# Key Decisions

- Not every superficially-similar status string should reuse the same mapping table. `permission_modal.tsx`'s
  status badge and `acp_debug.tsx`'s permission toggle share ACP tool-call vocabulary with
  `formatToolCallStatus`, so they call it directly. `output_header.tsx`'s merged status label uses agent
  lifecycle vocabulary (`idle`, `missing`, `working`, ...), a different source vocabulary that
  `formatToolCallStatus`'s tool-call-specific switch doesn't cover correctly (its default case would pass
  `"idle"` through unchanged) -- it got its own one-line capitalizer instead of a forced reuse.
- Focus trap/return behavior itself is delegated entirely to `@mantine/hooks`' `useFocusTrap`/
  `useFocusReturn` (already used elsewhere in the dependency tree), not reimplemented. The integration
  surface that needed verifying was "wired correctly, doesn't crash, existing tests still pass" -- the
  trap/return mechanics themselves are Mantine's tested responsibility.
- Confirmation dialogs use the existing `window.confirm` convention (matching "Delete Team"/"delete
  channel") rather than introducing a new confirm-modal component, to keep this a small, reviewable,
  behavior-consistent change rather than a design-system addition.
- Scope was deliberately capped at the review's top six findings. A full mechanical sweep of every admin
  button's loading state, the composer/markdown duplication cleanup, and the contrast/fixed-width
  accessibility findings are real but separate follow-ups (see Follow-Ups), not folded in here, to keep
  this changeset reviewable.
- No new test scaffolding was built for `onDeleteAgentNode` (node removal) or
  `onDeleteSelectedTeamAgent` (Team agent removal) confirmations -- neither had an existing test harness
  to extend cheaply. Verified via `tsc --noEmit` and the full existing suite passing unchanged; flagged
  here rather than silently skipped.

# Validation

- `npx tsc --noEmit` clean.
- `npm exec vitest -- run` -- 1515 passed (161 files; +5 new tests over the pre-change 1510), no
  regressions.
- `npm run lint` clean.
- `npm run build` succeeds.
- New regression tests: `create_agent_modal.test.tsx` (formError renders/doesn't render),
  `use_app_admin.test.tsx` (declining the confirmation blocks delete/revoke/bulk-delete),
  `admin_page_sections.test.tsx` (empty-state copy for all three lists), `join_page.test.tsx` (busy button
  text + disabled state + a second click while pending does not fire a duplicate
  `acceptTeamspaceInvite` call), `output_header.test.tsx` (updated to assert the now-humanized "Running"
  instead of the previously-buggy lowercase "running").

# Follow-Ups

Remaining findings from the same review round, not addressed here:

- Consistency/spec debt: `team_markdown.ts` duplicates `markdown.ts`'s LRU cache/`sanitizeHref`/autolink
  logic instead of sharing it; `InputDock` and `TeamMessageComposer` use two different color-token
  systems and send-button shapes, contradicting the spec's "one composer language" rule; channel-feed
  author name renders heavier than message body text (spec says metadata should be lighter);
  `rich_text_classes.ts` styles both `.md-*` semantic classes and raw tag selectors in parallel.
- Accessibility: muted text tokens (`notion-text-muted`, `ui-text-muted`) sit at the WCAG AA 4.5:1 floor
  and are used at 10-11px sizes with no large-text relief; `min-w-[220px]` fixed-width blocks inside
  popovers/menus can force horizontal scroll on ~320px viewports.
- `prefers-reduced-motion` is described in `frontend-design.md` as a requirement but has zero
  implementation anywhere in `web/src` -- the spec claim itself needs either real implementation or
  correction.
- Loading-state gaps remain on most `admin_page_sections.tsx` buttons (Add Path, Rotate Keys, Create
  Join Token, etc.) beyond the destructive ones fixed here.
- Auto-collapse via `IntersectionObserver` in `acp_tool_fold.tsx` silently closes a manually-opened tool
  card the instant it scrolls out of view.
