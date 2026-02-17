# Agents Interaction Polish

## Summary

Improve three high-friction areas in Agents mode:

1. Pending ACP permissions are surfaced as red indicators in both collapsed and expanded agents list views.
2. Terminal output auto-follow is more reliable when new output updates reuse existing event ids.
3. Agent switch latency is reduced by reusing the first fetch response instead of waiting for a second session-scoped fetch.

## Background

During daily usage, three issues repeatedly hurt usability:

- Permission requests were easy to miss when the agents panel was collapsed.
- Long-running terminal output sometimes stopped staying at bottom unless the user manually clicked the jump control.
- Switching to an agent with no cached explicit session required an extra roundtrip before output became visible.

## Scope

- `web/src/app.tsx`
- `web/src/components/agents_panel.tsx`
- `web/src/styles.css`
- `web/src/agents_panel.test.tsx`
- `web/src/app.permission_scope.test.ts`
- `web/src/output_cache.test.ts`
- `docs/todo.md`

## Key Decisions

1. Add a per-agent pending permission count map and derive global pending state from it.
2. Render visual pending indicators in two places:
   - collapsed rail (`Agents` metric),
   - expanded agent row (per-agent dot).
3. Change terminal auto-follow effect dependency from `outputs.length` to `outputs` to react to in-place message updates.
4. In `loadAgentEvents`, resolve latest session from the first response and immediately hydrate scoped cache/output state for that session key, avoiding a blank/slow interim state.

## Validation

```bash
npm --prefix web run lint
npm --prefix web run test -- src/agents_panel.test.tsx src/app.permission_scope.test.ts src/output_cache.test.ts
npm --prefix web run build
```

Expected outcomes:

- Lint passes.
- Added/updated tests pass.
- Production build succeeds.
- Latest-session hydration resolves from ordered events without requiring a second
  fetch pass when switching to `latest`.
- In-place output updates (same event id, new payload) keep a new output array
  reference so terminal follow effects can react consistently.

## Follow-ups

- Add a browser-level interaction test that verifies collapsed/expanded pending dots update when permission status changes.
