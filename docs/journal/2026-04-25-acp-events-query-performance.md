## ACP Events Query Performance

This change tightens ACP event history loading on both sides of the request path.

### Problem

- The Team ACP history prefetch path could raise `limit` up to `240` for chunk-heavy sessions.
- The `/api/agents/:id/events` query relies on `agent_events(session_id, id)` ordering, but older databases could lose the supporting indexes during the `message` column migration because the migration recreated the table and dropped the old indexes.

### Changes

- Lower the ACP history page-size cap from `240` to `180`.
- Recreate `agent_events` indexes after the message-column migration.
- Apply the same index-repair logic to per-agent event databases during schema initialization.

### Validation

- `cargo test -p agenthub-db init_db_migrates_agent_events_message_column_to_blob -- --nocapture`
- `cargo fmt --all --check`
- `cd web && pnpm exec vitest run src/pages/team/use_team_actions.test.tsx`
- `cd web && npm run build`
