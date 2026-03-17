# 2026-03-17 Team Runtime Action Button Wrap

## Summary

- Fixed the Team workbench muted action buttons rendering as white text on a white surface.
- Kept the Team header runtime controls from collapsing under tighter layouts at the same time.

## Why

The muted Team action buttons reused a custom light surface class on top of Mantine's default
filled button variant. Mantine still injected `--button-color: var(--mantine-color-white)` inline,
so the final result became a white background with white text.

The live regression was visible on the Team workbench header after the Team entered the running
state:

- `Stop Team` rendered as a blank white pill even though the DOM still contained the text;
- the same bug also affected other muted Team buttons such as `Team Selector` and modal `Cancel`.

The header controls also benefit from being explicitly non-shrinking so runtime badges do not force
the button labels into an unstable layout.

## What Changed

- `web/src/pages/team_page.tsx`
  - replaced the Mantine `Group` wrapper around the runtime action buttons with an explicit flex
    wrapper that still wraps across lines;
  - added a dedicated non-shrinking, no-wrap action-button class for the Team header controls;
  - switched the muted Team buttons to `variant=\"default\"` so Mantine emits a dark foreground
    color instead of the default filled-button white text;
  - applied the layout fix to `Add Agent`, `Start Team`, and `Stop Team`.

## Validation

- `cd web && npm run lint -- src/pages/team_page.tsx`

## Chrome MCP

- Live baseline was inspected on `https://agenthub.hawkingrei.com/teams/<team_id>`.
- Chrome DevTools MCP confirmed the running Team page still exposed `Stop Team` in the accessibility
  tree, while the visual screenshot showed a blank white button. Inspecting the live button DOM
  showed Mantine had injected `--button-color: var(--mantine-color-white)` inline on the muted
  button root.
- Post-edit live regression remains blocked until this change is deployed because verification must
  stay on the production domain.
