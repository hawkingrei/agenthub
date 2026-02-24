# Frontend Quality And Refactor

## Background

The main SPA entry combined routing, admin, join, auth, and ACP UI logic in a single file. Styling also lacked shared focus and spacing tokens. This made iteration slower and increased the chance of UI regressions.

## Scope

- Extract admin, join, and auth pages into dedicated modules.
- Move WebAuthn and push subscription helpers into shared utilities.
- Establish basic design tokens and consistent focus-ring styling.
- Extract agent list, input dock, and modal views into components.
- Extract ACP panel, conversation, debug, and terminal output views into components.
- Extract ACP conversation state management into a dedicated hook.
- Keep the existing visual direction while improving consistency and maintainability.

## Key Decisions

- Use `pages/` modules to separate route-level UI from the main app shell.
- Keep `app.tsx` as the orchestration layer for auth state and routing decisions.
- Add lightweight CSS tokens and focus styles without introducing a new styling system.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Confirm admin, join, and login flows render correctly after refactor.
- Evaluate additional component extraction for the Agents and Output panels.
- Validate Agent list actions, permission modal, and input dock behavior.
