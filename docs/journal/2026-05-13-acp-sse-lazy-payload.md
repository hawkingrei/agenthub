# ACP SSE Lazy Payload

## Summary

Large ACP tool-call payloads are now compacted before they are sent over SSE.
The persisted agent event remains complete in SQLite, and active ACP consumers can hydrate the
full event through a targeted event API when the UI actually needs the large fields.

## Background

ACP tool events can include large `content`, `raw_input`, and `raw_output` fields. Pushing those
fields through SSE is wasteful when the browser is closed, when a panel is inactive, or when the
client only needs metadata to keep the live stream ordered.

## Scope

- Compact large ACP `tool_call` and `tool_call_update` SSE messages by removing large payload
  fields and adding deferred metadata.
- Keep SQLite `agent_events` as the complete source for live event replay and lazy hydration.
- Add a single-event API for retrieving the exact persisted event by `(agent_id, event_id)`.
- Hydrate deferred Team member ACP events only while the member ACP consumer is active.

## Key Decisions

- The compaction boundary is the SSE transport, not persistence. Agent event storage still writes
  the complete compressed payload to the per-agent SQLite event database.
- Deferred SSE events keep identifying metadata inline and add `deferred_event_id`,
  `deferred_fields`, and `deferred_reason` so clients can decide whether to fetch the full event.
- The event API returns the persisted event rather than reconstructing ACP payloads from archive
  data.
- LanceDB remains an archive/search plane for message-shaped history. This change does not add a
  second live write of ACP SSE payloads into LanceDB.

## Validation

```bash
cargo fmt --all --check
cargo test -p agenthub compact_output_for_sse -- --nocapture
cd web && npm run test -- use_team_member_acp_effects.test.tsx
cd web && npm run lint
cd web && npm run build
```

## Follow-Ups

None for this slice.
