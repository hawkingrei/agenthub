# ACP Tool-Call Summary Top Alignment

## Background

After ACP conversation shell migration and follow-up style rollback, tool-call
summary rows still used centered alignment in base CSS. This made status marker,
title, and status badge appear vertically centered instead of top-aligned.

## Scope

- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Keep semantic-class styling path unchanged:
   - no additional Tailwind utility overrides for tool-call summary rows
2. Apply minimal CSS-only correction:
   - `.acp-tool-fold summary` -> `align-items: flex-start`
   - `.acp-tool-group-fold summary` -> `align-items: flex-start`
   - `.acp-tool-fold summary .acp-tool-title` -> `align-items: flex-start`
3. Add explicit left-alignment guards to avoid future centering regressions:
   - summary rows use `justify-content: flex-start` + `text-align: left`
   - status chips use `margin-left: auto` + `align-self: flex-start`
4. Fix grouped status chip tone semantics:
   - `N completed` uses success tone (green)
   - `N failed` uses failure tone (red)
   - `N running` uses running tone (blue)
5. Prevent grouped child tool-call cards from overflowing parent width:
   - enforce `box-sizing: border-box` on `tool_call`/`tool-group-entry`
   - enforce `min-width: 0` on group list and group item wrappers
6. Keep grouped status labels on one line:
   - apply `white-space: nowrap` + `word-break: keep-all` to tool status chips
7. Ensure ACP bubble content is explicitly left-aligned:
   - apply `text-align: left` on `.acp-bubble` base style to avoid inherited centering.
8. Ensure grouped title strings remain left-aligned under long content:
   - set `.acp-tool-group-title` to flex-start with full available width.
9. Remove horizontal whitespace on ACP conversation body:
   - remove conversation container horizontal padding
   - remove inner `max-width` cap so bubbles can use full body width
10. Keep fold behavior and animation styles unchanged.

## Validation Evidence (local)

- `npm --prefix web run test -- src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/acp_panel.test.tsx src/acp_debug.test.tsx`

## Follow-up Validation

- Manual desktop/mobile checks:
  - single `tool_call` summary row top alignment
  - grouped `tool_call_group` summary row top alignment
  - `explore_group` summary row top alignment

## Verification Result

- Local automated checks passed (ACP-focused tests).
- Manual visual verification confirmed by review feedback: all message bubbles
  look acceptable after alignment and width updates.
