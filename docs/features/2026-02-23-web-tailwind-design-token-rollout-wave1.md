# Web Tailwind Design Token Rollout (Wave 1)

## Background

Recent Team/ACP UI work still had two maintainability gaps:

1. long utility-class strings repeated in multiple components;
2. direct color/spacing literals (`slate-*`, `amber-*`, fixed paddings) spread across reusable class presets.

This made global style adjustments expensive and increased review noise.

## Scope

Wave 1 standardizes shared style presets for high-traffic web surfaces:

- Team workbench sidebar and Team create flow presets
- ACP panel/debug/conversation presets
- Agents list panel presets
- Output header/body presets
- Input dock presets

No runtime behavior or interaction contract changed.

## Key Decisions

1. Introduce semantic Tailwind tokens in `web/tailwind.config.cjs`:
   - `brand.*` for primary brand surfaces
   - `ui.*` for neutral surface/border/text system
   - `state.*` for info/warning/success states
   - `spacing.ctrl-*` and `fontSize.ui-*` for control density consistency
2. Keep utility-first consumption through `web/src/ui/tailwind_classes.ts` instead of adding new handcrafted global CSS blocks.
3. Migrate component-local class constants into centralized presets to reduce inline repetition and improve change locality.

## Files Changed

- `web/tailwind.config.cjs`
- `web/src/ui/tailwind_classes.ts`
- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `web/src/components/output_body.tsx`
- `web/src/components/input_dock.tsx`
- `web/src/pages/team_sidebar.tsx`

## Validation

Local validation performed:

- `pnpm -C web run lint`
- `pnpm -C web run build`
- `pnpm -C web exec vitest run src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_conversation_render.test.tsx`
- `pnpm -C web exec vitest run src/agents_panel.test.tsx src/output_header.test.tsx src/output_body.test.tsx src/input_dock_render.test.tsx src/input_dock_keyboard.test.ts src/pages/team_panels.test.tsx`

## Follow-ups

- Continue token migration for remaining Mantine-adjacent pages where shared class presets are still duplicated.
- After merge, record push + PR CI run IDs in PR evidence before marking the TODO item done.
