# Web Tailwind UI Phase-8: ACP Panel + ACP Debug Shell

## Background

After Team shell migration, ACP surfaces still depended on legacy global CSS for
top-level container and debug controls.

This phase migrates ACP shell styling to Tailwind utilities while preserving
existing debug and permission interaction behavior.

## Scope

- `web/src/components/acp_panel.tsx`
- `web/src/components/acp_debug.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep ACP behavior unchanged:
   - conversation/debug tab switching
   - session control callbacks (`set mode/model/config`, cancel, clear)
   - permission jump/copy flows
   - runtime/raw event rendering semantics
2. Preserve test-coupled semantic classes and add utilities on top:
   - keep `tab`, `active`, `acp-permission-copy`, and existing ACP semantic hooks
   - avoid breaking interaction tests that query class selectors directly
3. Introduce local utility class constants for ACP debug controls:
   - debug tabs
   - input fields
   - primary/secondary action buttons

## Validation Evidence (local)

- Focused ACP tests:
  - `npm --prefix web run test -- src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_debug.interaction.test.tsx src/acp_debug_permissions.test.ts`
- Workspace regression tests:
  - `npm --prefix web run test -- src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx src/agents_panel.test.tsx src/output_header.test.tsx src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_debug.interaction.test.tsx src/acp_debug_permissions.test.ts`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/` ACP mode:
  - tab readability and active-state contrast
  - session control layout wrapping on small screens
  - permission row jump/copy affordance
  - raw event list scrolling and payload readability

## Notes

- This phase intentionally does not alter ACP data model, copy payload content,
  or permission jump resolution logic.
