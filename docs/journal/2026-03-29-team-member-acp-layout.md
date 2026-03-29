## Why

The Team member ACP panel was still using a mixed container layout where the member header and ACP
body shared a plain block wrapper. In constrained layouts this let the ACP body compete with the
input dock for height, which could visually collapse or overlap the member info/header area with
the composer region.

## What Changed

- changed the Team member ACP shell to a dedicated `flex-col` layout;
- kept the member header as a shrink-fixed row;
- wrapped the ACP body in its own `min-h-0 flex-1 overflow-hidden` container so the conversation
  scroll region owns the remaining height cleanly above the input dock;
- added a focused regression test for the Team member ACP shell structure.

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
- `git diff --check`
