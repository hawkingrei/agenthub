---
title: ACP Permission Events In Debug Stream
date: 2026-02-09
status: implemented
---

## Summary

Emit explicit ACP debug events when permission requests are created, resolved,
or timed out, so the Debug tab can show the full permission lifecycle.

## Background

Operators could not see permission prompts in the Debug stream, making it
difficult to understand why a tool call was blocked or timed out.

## Decision

- Emit `permission_request` when a permission prompt is created.
- Emit `permission_response` when the outcome is decided.
- Emit `permission_timeout` when a request expires and the fallback is applied.
- Include `permission_id`, `session_id`, `tool_call_id`, outcome, and timestamps.

## Scope

- `src/acp.rs`

## Validation

- Trigger a permissioned tool call and verify the Debug raw events list shows
  request/response entries with matching IDs.
- Force a timeout and confirm a `permission_timeout` event is recorded.
