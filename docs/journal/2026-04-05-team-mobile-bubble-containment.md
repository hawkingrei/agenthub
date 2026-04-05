# fix(web): keep team chat bubbles inside mobile viewport

## Summary

Constrain Team conversation bubbles so long markdown tokens and fenced code blocks do not stretch past the mobile viewport.

## Changes

- add `overflow-hidden` to the Team chat bubble shell so descendant content cannot visually spill past the rounded bubble frame
- add mobile-safe markdown constraints to Team chat rich text:
  - `max-w-full`
  - `overflow-wrap:anywhere`
  - descendant `pre` elements wrap long lines instead of widening the bubble
  - descendant table cells and list content can wrap inside the bubble

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/ui/tailwind_classes.ts src/pages/team_panels.test.tsx`
- `make build-web`

- apply the same mobile-safe rich-text constraints to shared ThreadRichText so ACP and Team chat use one containment contract

## Behavior

- Long lines now wrap inside the bubble instead of forcing horizontal overflow.
