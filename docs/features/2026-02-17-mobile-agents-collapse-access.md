# Mobile Agents Collapse Access

## Summary

Restore an always-clickable `Hide agents` control when the agents panel is expanded on narrow viewports.

## Background

On mobile breakpoints, expanded agents panel uses a fixed overlay (`workspace-left` above `workspace-right`).  
The existing collapse control in `OutputHeader` remains rendered but is visually and interactively covered by the overlay, so users can miss the collapse affordance.

## Scope

- `web/src/components/agents_panel.tsx`
- `web/src/styles.css`
- `web/src/agents_panel.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Add a dedicated `Hide agents` button inside the expanded agents panel toolbar.
2. Keep this button responsive-only (`max-width: 1024px`) to avoid duplicating desktop controls.
3. Add a render assertion in `agents_panel.test.tsx` to prevent regression.

## Validation

```bash
npm --prefix web run test -- agents_panel.test.tsx
npm --prefix web run build
```

Expected outcomes:

- Expanded panel markup includes `aria-label="Hide agents"`.
- Web build stays green.

## Follow-ups

- Add a browser interaction test for mobile overlay mode that verifies `Hide agents` inside panel closes the drawer and reveals output area controls.
