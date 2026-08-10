# Workspace UI Compaction Wave 2

## Summary

Compacted a residual workspace shell UI journal cluster into the canonical unified workspace IA
spec. The stable rules for shell density, channel-first lens language, Team/Agent rail chrome, ACP
embedded labels, message metadata, and composer hint copy now live in
`docs/features/workspace-unified-ia.md`.

## Background

Several April workspace UI journals recorded small Notion-density and Slock-reference passes. They
were valuable rollout evidence, but their stable rules had drifted across multiple dated notes.
This made the current contract harder to find when changing the workspace shell.

## Scope

This compaction pass covers:

- `2026-04-18-workspace-shell-compactness.md`
- `2026-04-18-workspace-slock-notion-density-pass.md`
- `2026-04-19-workspace-agent-pane-chrome-tightening.md`
- `2026-04-19-workspace-channel-first-lens-language.md`

It updates only documentation. It does not change web runtime behavior.

## Key Decisions

- The canonical workspace shell should behave like a compact object directory plus one active work
  surface, not like a status dashboard.
- `Channels / Tasks / Members / Search` remains the workspace-global lens language.
- `thread` remains subordinate to `channel`; compatibility values such as `chat` and `threads`
  should normalize to `channels`.
- Team and Agent rail rows should prefer `title + one compact meta line` density.
- Agent/ACP embedded panes should keep runtime depth while presenting lighter page-tab and metadata
  chrome.
- Old journals keep rollout evidence, while stable rules move to the canonical spec.

## Validation

Focused checks for this documentation slice:

```bash
git diff --check
```

## Completion

The Team conversation/composer and ACP-heavy UI clusters were subsequently compacted, and the full
wave is closed in
[`2026-08-09-feature-docs-compaction-wave2-closeout.md`](2026-08-09-feature-docs-compaction-wave2-closeout.md).

## Follow-Ups

None for this documentation cluster.
