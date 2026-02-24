---
title: ACP Chunk Stitching for Kimi/Gemini
date: 2026-02-10
status: implemented
---

## Summary

Fill in missing `message_id`/`chunk_index` for ACP chunks and improve tool call
content stitching so chunked output renders cleanly.

## Background

Kimi/Gemini ACP output may emit chunked messages without `message_id` or
`chunk_index`, which prevents the UI from merging chunks. Tool call updates also
emit partial text fragments, leading to noisy content.

## Decision

- Generate synthetic `message_id` and `chunk_index` for ACP message chunks when
  metadata is missing, resetting on non-message events.
- Extract text blocks from tool call content and prefer the longest fragment so
  incremental updates show a coherent command.

## Scope

- `src/acp.rs`
- `web/src/acp.ts`
- `web/src/acp.test.ts`

## Validation

- [ ] Start a Kimi/Gemini agent and confirm agent messages are merged into full
  sentences instead of per-character chunks.
- [ ] Confirm tool call content shows full commands as updates arrive.
