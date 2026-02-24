# Web Tailwind UI Phase-5: AgentsPanel + OutputHeader

## Background

After migrating Team workbench shells, the main agents workspace still relied on
legacy styling in `AgentsPanel` and `OutputHeader`. This phase introduces
Tailwind utility shells to these two high-visibility components.

## Scope

- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep all interaction logic unchanged:
   - collapsed/expanded sidebar switching
   - create/select/start/stop/delete/code-mode actions
   - output header subtitle and ACP visibility behavior
2. Preserve existing semantic class names used by tests and legacy CSS, and layer
   Tailwind utility classes on top.
3. Add utility-driven styling for:
   - sidebar shell and toolbar spacing
   - create button and agent row surface states
   - output header shell, meta layout, and code-mode chip styling

## Validation Evidence (local)

- Focused component tests:
  - `npm --prefix web run test -- src/agents_panel.test.tsx src/output_header.test.tsx`
- Team panel regression check:
  - `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/`:
  - agents collapsed rail + expanded sidebar transitions
  - active row visibility and action button affordance
  - output header subtitle shown only when ACP is absent
  - header metadata readability under long agent names/model labels

## Notes

- This phase intentionally does not alter ACP/terminal logic or session metadata
  derivation.
