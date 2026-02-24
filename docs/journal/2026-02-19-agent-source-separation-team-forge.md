# Agent Source Separation For Team Forge

## Background

Team Forge and Agents page previously shared the same `POST /api/agents` write path and
the `agents` table had no source marker. This made Team Forge-created agents appear in
the main Agents list, which broke expected UX separation.

## Scope

- Add a source marker for agents (`manual` / `team_forge`).
- Keep manual-created agents visible in `/api/agents`.
- Hide Team Forge-created agents from `/api/agents` list output.
- Keep backward compatibility for existing databases that may not have the new column.

## Key Decisions

1. Persist source as `agents.source` (`TEXT NOT NULL DEFAULT 'manual'`).
2. Keep create API backward compatible:
   - missing `source` defaults to `manual`;
   - Team page sends `source: "team_forge"` explicitly for forge flows.
3. Filter `/api/agents` by source (`source != 'team_forge'`), while preserving existing
   runtime-hide rule for active team member sessions.
4. Keep old test/local DB compatibility by checking `agents.source` column existence in
   manager paths and falling back when absent.

## Validation

- `cargo test -p agenthub parse_agent_source_defaults_and_validates -- --nocapture`
- `cargo test -p agenthub list_agents_hides_team_forge_source_agents -- --nocapture`

## Manual DB Upgrade (if needed)

For a running local DB that predates this change, apply:

```sql
ALTER TABLE agents ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
```

This is also covered by startup compatibility logic in `src/db.rs`.
