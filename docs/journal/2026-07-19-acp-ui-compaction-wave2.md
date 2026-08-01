# ACP UI Compaction Wave 2

## Summary

Compacted the ACP-heavy UI journal cluster into canonical feature specs. Stable rules for ACP
tool-call folding, grouped tool calls, humanized payload rendering, markdown safety, mobile ACP
headers, debug-shell controls, semantic classes, and progressive/virtualized conversation rendering
now live in `docs/features/acp-runtime.md` and `docs/features/frontend-design.md`.

## Background

The remaining features compaction wave 2 work was ACP-heavy UI history. The relevant February notes
recorded important rollout evidence, but the stable behavior had become spread across dated files
instead of the ACP and frontend feature specs.

## Scope

This compaction pass covers:

- `2026-02-13-acp-ui-fold-markdown-mobile.md`
- `2026-02-15-acp-tool-call-humanized-rendering.md`
- `2026-02-17-acp-tool-call-group-fold-animation.md`
- `2026-02-20-web-tailwind-ui-phase8-acp-panel-debug-shell.md`
- `2026-02-20-web-tailwind-ui-phase9-acp-conversation-shell.md`

It updates only documentation. It does not change ACP runtime behavior or provider integration.

## Key Decisions

- ACP conversation/debug UI rendering remains part of the ACP contract because it governs
  inspectability for long provider runs.
- Tool calls should stay foldable, groupable, and jump-addressable without losing nested details.
- Structured tool payloads should default to human-readable key/value and nested fold views instead
  of raw JSON-first blocks.
- Markdown, terminal/ANSI output, diff output, and ASCII-like multiline output need stable safe
  rendering rules.
- ACP Debug remains visually secondary to Conversation while keeping permission, runtime metrics,
  raw event, jump, copy, and session-control affordances available.
- Semantic classes used by tests and compatibility selectors remain part of the UI stability
  contract when Tailwind utilities are layered on top.

## Validation

Focused checks for this documentation slice:

```bash
git diff --check
```

## Follow-Ups

- Future ACP UI changes should update `docs/features/acp-runtime.md` or
  `docs/features/frontend-design.md` directly, then add a dated journal only for rollout evidence.
- Provider/runtime integration evidence remains separate from UI compaction evidence.
