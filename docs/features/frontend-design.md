# Frontend Design Specification

## Problem

The web layer evolved quickly across ACP, Team, and Agents surfaces, but styling and interaction contracts
were spread across many date-based notes. Reuse and consistency risks increased when utility classes,
state labels, and panel behaviors diverged.

## Scope

- Design system conventions for Team/ACP/Agents surfaces.
- Interaction contracts for conversation-first Team UX and debug affordances.
- Shared class/token strategy for maintainability.

## Non-Goals

- Re-documenting every UI migration phase in timeline form.
- Replacing component-level tests with this spec.
- Defining backend business rules.

## Architecture

### 1) Design System Baseline

- UI component baseline: `@mantine/core` + Tailwind CSS utilities.
- Styling strategy:
  - semantic tokens in Tailwind config;
  - matching `@theme` token definitions in `web/src/tailwind.css`;
  - reusable class presets in `web/src/ui/tailwind_classes.ts`;
  - thin shared UI primitives in `web/src/ui/primitives.tsx` and
    `web/src/ui/floating_surfaces.ts`;
  - avoid new handcrafted global CSS blocks.

### 1.1) Workbench Shell Language

- Team, ACP, and Agents workbenches should read as intentionally separated boxes instead of one
  continuous canvas.
- Prefer Bento-style composition for multi-panel screens:
  - one dominant primary box for the current task lane;
  - smaller secondary boxes for navigation, runtime context, or debug affordances;
  - visible spacing between boxes so hierarchy stays legible at a glance.
- Prefer Neo-Minimalist surface treatment:
  - low-saturation backgrounds;
  - restrained borders and shadows;
  - typography and spacing should carry hierarchy before color does.
- Keep the visual language operational rather than decorative: panels should feel like a calm
  control surface, not a marketing layout.

### 2) Primary Team UX

- Team surface is conversation-first for human interaction.
- `Conversation` remains available even when no active run exists.
- Conversation uses one shared group stream (human + coordinator + workers); no per-recipient split view on the human surface.
- Shared conversation and ACP views are intentionally bounded recent windows by
  default (current product baseline: recent-10) instead of infinite in-memory
  scrollback.
- Default response routing is team-wide when no `@mention` is provided, with coordinator-first speaking priority.
- `@member_id` marks people in the same shared stream and should be surfaced back to agents/UI as mention metadata, without changing group-chat fan-out.
- `Runs` is the dedicated run-entry tab (run browser + `Start Team` + run selection).
- Internal execution controls (`Run Ops`, `Step Ops`, raw mailbox tools) remain in Debug sections.
- `Start Team` stays visible as explicit operator action.

### 3) Team Navigation Composition

- Team tab routing should be driven by shared state metadata instead of duplicated inline conditionals.
- Shared composition primitives:
  - `TeamTabsBar` for top-level tab rendering.
  - `TeamActiveRunPanel` for active-run context/actions across run-scoped tabs.
- Run-required tabs should share one gate policy and one fallback card (`No Active Run` + `Go to Runs`).

### 4) Team Status Visibility

- Team main view should expose member lifecycle summary and per-member status.
- User-facing lifecycle buckets:
  - `working`
  - `idle`
  - `stopped`
  - `missing`

### 5) Accessibility And Mobile Guardrails

- Keep high-frequency controls reachable on narrow viewports.
- Preserve keyboard/touch parity for input, send, interrupt, and jump-to-bottom actions.
- Avoid overflow clipping in Team/ACP panels and message lists.

## Contracts

### 1) Styling Contract

- New reusable visual patterns should be extracted into shared Tailwind class presets.
- Repeated shells/actions/badges should prefer shared UI primitives before
  introducing new page-local Tailwind strings.
- Feature code should prefer semantic tokens (`brand.*`, `ui.*`, `state.*`) over literal color strings.

### 2) Team Interaction Contract

- Human-facing Team terminology should prioritize `Conversation` instead of exposing internal `task` wording.
- Debug-only operations keep internal runtime concepts and IDs.
- `@mention` semantics should be explicit in UI copy:
  - no mention -> whole team target;
  - no mention -> coordinator should respond first;
  - mention(s) -> prioritized responders;
  - worker should normally wait unless correcting, supplementing, or reporting new findings;
  - all messages remain visible in the shared group conversation.

### 3) Team Tab Routing Contract

- Tab definitions (`value`, `label`) should be centralized in Team state metadata.
- Active-run requirement policy should be centralized and reusable by page/panel composition.
- Conversation lane must not be blocked by run-loading or run-selection requirements.

### 4) Status Presentation Contract

- Rendering layers should map runtime statuses into stable user buckets.
- `unknown` should be treated as diagnostic state and not the default user-facing label.

## Validation Matrix

- `pnpm -C web run lint`
- `pnpm -C web run build`
- `pnpm -C web exec vitest run src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts src/pages/team/state.test.ts`
- `pnpm -C web exec vitest run src/pages/team_member_status_strip.test.tsx`

## Operational Notes

- Keep shared visual primitives centralized to lower refactor cost.
- MCP/UI smoke checks should validate `/teams` main flow plus ACP/Agents critical controls after significant UI changes.

## Open Risks

- Long-tail legacy selectors may still appear in edge pages.
- Some mobile layouts can regress under new content density unless guarded by viewport tests.

## Next Milestones

### Tailwind Wave 2

#### Goals

- Complete semantic-token adoption for remaining high-traffic pages and remove duplicated utility bundles.
- Reduce style drift between Team, ACP, and Agents by converging on shared class presets.
- Keep migration behavior-preserving (no API/state contract changes).

#### Scope (Wave 2)

- Expand `web/src/ui/tailwind_classes.ts` presets to remaining page shells and repeated control groups.
- Replace remaining literal color/spacing utility strings with semantic tokens from `tailwind.config.cjs`.
- Continue shrinking legacy global selectors in `web/src/styles.css` with compatibility-safe isolation.
- Add focused visual regression coverage for mobile breakpoints where Tailwind migrations are most fragile.

#### Execution Plan

1. Inventory duplicated class clusters and rank by frequency/risk.
2. Migrate one surface group per PR (small blast radius, easier rollback).
3. Attach per-PR before/after MCP snapshots and targeted test evidence.
4. Run compaction pass to remove superseded style helpers and stale selectors.

#### Acceptance Criteria

- No new handcrafted global CSS blocks.
- Shared token/preset usage increases on all migrated surfaces.
- Existing Team/ACP/Agents interaction tests remain green.
- `pnpm -C web run lint` and `pnpm -C web run build` remain green on each migration PR.

## Source Journals

- `docs/journal/2026-02-23-web-tailwind-design-token-rollout-wave1.md`
- `docs/journal/2026-02-21-team-usability-polish-and-scroll-hardening.md`
- `docs/journal/2026-02-24-team-task-human-conversation-ui.md`
- `docs/journal/2026-02-23-team-top-member-status-strip.md`
- `docs/journal/2026-02-25-team-runs-tab-and-tab-routing-refactor.md`
