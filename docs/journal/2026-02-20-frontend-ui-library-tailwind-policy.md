# Frontend UI Library + Tailwind Policy

## Background

To keep AgentHub frontend implementation consistent and maintainable, we need a
single default UI implementation path for all follow-up UI work.

## Scope

- `AGENTS.md`
- `docs/todo.md`

## Key Decisions

1. Set a frontend UI standard in `AGENTS.md`:
   - use `@mantine/core` components plus Tailwind CSS utility classes for new UI
     features and UI refactors.
2. Add a CSS guardrail in `AGENTS.md`:
   - avoid introducing new large handcrafted global CSS blocks;
   - limit legacy `web/src/styles.css` edits to compatibility fixes during
     migration.
3. Add an explicit requirement addition in `AGENTS.md` so this policy is visible
   in the latest project context section.
4. Track policy enforcement with a dedicated TODO item in `docs/todo.md`.

## Validation Plan

- Policy text verification:
  - confirm `AGENTS.md` includes both technical-constraint and requirement-addition
    entries for UI library + Tailwind policy.
- Process verification:
  - for upcoming PRs, review changed frontend files and ensure no new large
    handcrafted global CSS blocks are introduced outside compatibility fixes.

## Notes

- This is a policy/documentation change and does not directly alter runtime
  behavior.
- Existing legacy styles remain valid for incremental migration compatibility.
