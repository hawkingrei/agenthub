---
title: ACP Interrupt Enabled During In-Progress Tool Calls
date: 2026-02-09
status: implemented
---

## Summary

Enable the Interrupt button and runtime badge when ACP tool calls are in progress,
even if an explicit `run_status` event is missing.

## Background

Some ACP clients do not emit `run_status` events for tool execution. The UI
was gating the Interrupt button and runtime badge on `run_status`, which
left the controls disabled while tool calls were actively running.

## Decision

- Derive a `runStatusLabel` from `run_status` when available.
- Fall back to `in_progress` when any tool call is running.
- Use the derived label to enable Interrupt and display a status badge.

## Scope

- `web/src/components/acp_panel.tsx`
- `web/src/styles.css`

## Validation

- Trigger a tool call that stays `in_progress`; confirm Interrupt is enabled.
- Confirm the ACP status badge shows `in_progress` without `run_status`.
- Run `pnpm test` (or `npm test`) and confirm `acp_panel.test.tsx` passes.
