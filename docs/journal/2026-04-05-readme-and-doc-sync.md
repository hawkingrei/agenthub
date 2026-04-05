# README And Docs Sync

## Summary

- Updated `README.md` to reflect the current web workbench direction:
  installable PWA shell, Mantine + Tailwind UI baseline, shared UI primitives,
  and bounded recent-window conversation behavior.
- Updated `docs/README.md` so frontend/UI work explicitly points contributors to
  `docs/features/frontend-design.md` and the Team workbench user docs.
- Synced `docs/features/frontend-design.md` and
  `docs/features/agents-teams.md` with the current frontend architecture and
  Team conversation-window contract.
- Updated the user-facing Team workbench guide to describe the current
  recent-10 shared-channel behavior instead of the older “hydrate a larger tail”
  wording.

## Validation

- `npm --prefix userdocs run build`
