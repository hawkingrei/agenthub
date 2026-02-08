---
title: ACP Debug Scroll And Interrupt Control
date: 2026-02-08
status: implemented
---

## Summary

Expose a quick "Interrupt" control in the ACP header and ensure the Debug
panel raw events list can scroll independently.

## Background

Operators needed a faster way to cancel an ACP run while reviewing output,
and the Debug panel raw events list was difficult to access on smaller
screens without a dedicated scroll container.

## Decision

- Add an "Interrupt" button to the ACP header, positioned before the
  Conversation tab. The button is enabled only when the agent is running.
- Make the Debug panel a flex column and allow the raw events list to scroll
  within the ACP pane.

## Scope

- `web/src/components/acp_panel.tsx`
- `web/src/styles.css`

## Validation

- Manual: verify the Interrupt button disables when not running and sends
  cancel while a run is active.
- Manual: confirm the Debug tab lists raw events and scrolls within the panel.
