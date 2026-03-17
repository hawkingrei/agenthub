# Agents ACP debug terminal stderr

## Summary

- Problem: ACP-enabled agents already collected plain `stdout` / `stderr` / `system` lines, but the main workbench conversation intentionally focused on ACP events, so ordinary process output was only available inside `Debug -> Terminal`.
- Goal: keep the conversation pane clean while making `Debug -> Terminal` substantially better for inspecting plain `stderr`.

## Implementation

- Kept the ACP main conversation/plan views free of terminal output.
- Enhanced `Debug -> Terminal` with:
  - per-stream counts for `stdout`, `stderr`, and `system`;
  - quick filters for `All`, `Stderr`, and `System`;
  - empty-state copy when the selected filter has no matching lines.

## Validation

- Focused web tests should cover:
  - terminal summary renders counts and filter controls;
  - selecting `Stderr` hides non-error output and keeps only `stderr` lines visible;
  - ACP conversation body still does not inline terminal output.

## Follow-up

- If we later want stronger operator visibility, add a lightweight error badge that points users into `Debug -> Terminal`, rather than duplicating terminal output into the conversation pane.
