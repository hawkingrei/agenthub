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
  - reusable class presets in `web/src/ui/tailwind_classes.ts`;
  - avoid new handcrafted global CSS blocks.

### 2) Primary Team UX

- Team surface is conversation-first for human interaction.
- Internal execution controls (`Run Ops`, `Step Ops`, raw mailbox tools) remain in Debug sections.
- `Start Team` stays visible as explicit operator action.

### 3) Team Status Visibility

- Team main view should expose member lifecycle summary and per-member status.
- User-facing lifecycle buckets:
  - `working`
  - `idle`
  - `stopped`
  - `missing`

### 4) Accessibility And Mobile Guardrails

- Keep high-frequency controls reachable on narrow viewports.
- Preserve keyboard/touch parity for input, send, interrupt, and jump-to-bottom actions.
- Avoid overflow clipping in Team/ACP panels and message lists.

## Contracts

### 1) Styling Contract

- New reusable visual patterns should be extracted into shared Tailwind class presets.
- Feature code should prefer semantic tokens (`brand.*`, `ui.*`, `state.*`) over literal color strings.

### 2) Team Interaction Contract

- Human-facing Team terminology should prioritize `Conversation` instead of exposing internal `main_task` wording.
- Debug-only operations keep internal runtime concepts and IDs.

### 3) Status Presentation Contract

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

## Source Journals

- `docs/journal/2026-02-23-web-tailwind-design-token-rollout-wave1.md`
- `docs/journal/2026-02-21-team-usability-polish-and-scroll-hardening.md`
- `docs/journal/2026-02-24-team-main-task-human-conversation-ui.md`
- `docs/journal/2026-02-23-team-top-member-status-strip.md`
