# Team Conversation Composer Closeout

## Summary

The Team conversation/composer polish TODO is closed. Team channel rows, thread rows, and embedded
ACP conversation rows now share the stable chat-system contract in
`docs/features/frontend-design.md`: wide content lanes, neutral content-first bubbles, compact
metadata, and a lightweight composer language across channel, thread, and ACP input surfaces.

## Background

The 2026-04 Slock-style polish pass established the target look, and the 2026-07 compaction pass
moved the durable contract into the frontend design spec. This checkpoint adds the missing component
evidence for the ACP input dock so the TODO is no longer only supported by prose.

## Scope

- Keep the stable contract in `docs/features/frontend-design.md`.
- Guard the ACP input dock against drifting away from the Team channel/thread composer language.
- Close only the conversation/composer polish TODO.

## Key Decisions

- The channel and thread surfaces continue to use `TeamMessageComposer`.
- The ACP input dock remains a distinct runtime input component, but it reuses the same editor row,
  actions row, helper text, and send-button visual language through shared class presets.
- Message bubble identity stays neutral and content-first; author identity remains in metadata and
  avatar lanes instead of heavy author-color fills.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team/team_message_composer.test.tsx src/pages/team/team_thread_pane.test.tsx src/components/input_dock.test.tsx
```

## Follow-Ups

- Browser/profiler evidence for broad page-level performance remains tracked by the frontend
  performance hardening TODO.
