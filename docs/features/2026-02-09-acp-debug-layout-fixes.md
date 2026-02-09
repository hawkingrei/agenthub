# ACP Debug Layout Fixes

## Background
The ACP debug panel still failed to scroll reliably after recent layout refactors, and review feedback highlighted grid/flex mismatches and missing height constraints. Additional feedback covered interrupt gating, permission event serialization, and lock usage in agent input handling.

## Scope
- Define ACP container as a two-row grid (`auto` + `minmax(0, 1fr)`) so the active panel can scroll.
- Ensure `.acp-debug` has explicit height constraints and scroll behavior.
- Gate Interrupt to active runs only (running or tool call in progress).
- Emit permission debug events without moving `args.options` or outcomes.
- Release agent manager locks before performing async DB updates.
- Log tool output content deserialization failures instead of silently dropping them.

## Key Decisions
- Prefer a grid layout for ACP to keep header sizing stable while panels consume remaining space.
- Keep `runStatusLabel` for badge rendering, but restrict `canInterrupt` to active statuses.
- Log JSON deserialization errors with the raw value to aid protocol debugging.

## Validation
- Open ACP Debug and confirm Raw Events scrolls within the Output panel.
- Trigger a permission request and verify debug events emit without panics.
- Confirm Interrupt is disabled for completed/failed runs but enabled during in-progress tool calls.
- Run `cargo check` to ensure the agent manager and ACP handler compile.
