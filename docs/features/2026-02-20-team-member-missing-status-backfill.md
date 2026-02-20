# Team Member Missing Status Backfill

## Summary

Fix Teams page member lifecycle mapping so members are not incorrectly marked as
`missing` when `/api/agents` intentionally hides them.

## Background

Team members can be hidden from `/api/agents` by design:

1. `source=team_forge` agents are excluded from the general Agents page list.
2. Active team-member runtime agents (`working` / `input_required`) are excluded
   to avoid duplicate exposure in non-Team views.

Teams page previously reused `/api/agents` as its only member status source.
When a team spec member was filtered out of that list, UI mapped it to
`missing` even though `/api/agents/:id` still returned the agent.

## Scope

- `web/src/api.ts`
- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `docs/todo.md`

## Key Decisions

1. Keep `/api/agents` filtering behavior unchanged to preserve existing product
   boundaries for Agents page and Team runtime isolation.
2. Backfill unresolved team members on Teams page with targeted
   `/api/agents/:id` lookups keyed by `spec.members[].member_id`.
3. Cache lookup results in page state (`AgentRecord | null`) to avoid repeated
   N+1 requests for the same unresolved member IDs during one page lifecycle.
4. Extend helper-level unit tests to cover fallback mapping behavior.

## Validation

```bash
npm --prefix web run test -- team_page.runs.test.ts
```

