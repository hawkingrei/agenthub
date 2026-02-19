# Agent/Team Shared Status Design Language Foundation

## Summary

Introduce a shared status badge component and tone mapping so `Agents` and `Teams`
use the same status semantics and visual language.

## Background

`Agents` and `Teams` pages evolved separately:
- `Agents` used `agent-status` with page-local color mapping,
- `Teams` used `team-status` / `team-status-chip` with a different style contract.

This created inconsistent tone meaning (for example, `idle`/`submitted`) and made
future UI reuse harder.

## Scope

- `web/src/components/status_badge.tsx`
- `web/src/components/status_badge.test.ts`
- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Add shared `StatusBadge` component with explicit tones:
   - `neutral`, `active`, `inactive`, `warning`, `danger`.
2. Add shared tone mappers:
   - `resolveAgentStatusTone`
   - `resolveTeamLifecycleStatusTone`
   - `resolveTeamRunStatusTone`
3. Keep existing class names for compatibility (`agent-status`, `team-status`, `team-status-chip`),
   but unify styling under shared `status-badge` tone tokens.
4. Apply the shared component in three core surfaces first:
   - Agents list rows,
   - output header active-agent status,
   - team run/member status chips.
5. Use CSS variables for status colors to allow future theme/system-level alignment.

## Validation

Executed:

```bash
npm --prefix web run test -- src/components/status_badge.test.ts src/pages/team_page.runs.test.ts
npm --prefix web run lint
npm --prefix web run build
```

Re-verified on current branch:

- `npm --prefix web run test -- src/components/status_badge.test.ts src/pages/team_page.runs.test.ts` (28 passed)
- `npm --prefix web run lint` (pass)
- `npm --prefix web run build` (pass)

Manual checks to run in browser:

1. Open `/` and verify Agents list status badges use unified tone colors.
2. Open `/` and verify output header status badge matches Agents list tone.
3. Open `/teams` and verify run/member status badges follow the same tone system.
4. Check active row contrast (`team-item.active`) keeps badge readability.
