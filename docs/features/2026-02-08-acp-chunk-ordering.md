---
title: ACP Chunk Ordering Metadata
date: 2026-02-08
status: implemented
---

## Summary

ACP streaming updates now carry per-message ordering metadata (`message_id` and
`chunk_index`) so the UI can merge token-level chunks deterministically.

## Background

AgentHub persists ACP stream chunks as individual `agent_events`. With token-level
streams, ordering could drift when chunks are merged from different sources (SSE,
polling, local cache). Without chunk ordering metadata, the frontend could only
append consecutive chunks, which breaks on out-of-order arrivals or non-contiguous
chunks.

## Decision

- Emit `message_id` and `chunk_index` in ACP chunk `_meta` during streaming.
- Propagate these fields into the JSON payload stored and streamed by AgentHub.
- Merge chunked messages in the UI by `message_id` and reconstruct text by
  sorting `chunk_index` values.

## Scope

- `agenthub-codex-acp`: attach `_meta` to message and thought chunks.
- `agenthub` ACP bridge: include `message_id` and `chunk_index` in persisted JSON.
- `web`: merge ACP chunks by `message_id` and `chunk_index`.

## Validation

- `pnpm test` (or `npm test`) for `web/src/acp.test.ts`.
- Manual: stream a long ACP response, confirm text remains in order during SSE,
  polling, and after reload.
