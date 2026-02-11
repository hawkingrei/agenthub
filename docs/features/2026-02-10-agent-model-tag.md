---
title: Agent Model Tag
date: 2026-02-10
status: implemented
---

## Summary

Show a model tag next to agent names in the agents list and output header.

## Background

Users could not quickly see which model/runtime an agent was using from the
workspace UI, making it harder to differentiate Codex/Gemini/Kimi sessions.

## Decision

- Derive a display label from `--model`/`-m` arguments when present.
- Fall back to ACP provider inferred from the command, or the command name when
  the provider is unknown.
- Render the label as a compact tag next to the agent name in both the list and
  the output header.

## Scope

- `web/src/agent_presets.ts`
- `web/src/components/agents_panel.tsx`
- `web/src/components/output_header.tsx`
- `web/src/app.tsx`
- `web/src/styles.css`
- `web/src/agent_presets.test.ts`
- `web/src/output_header.test.tsx`

## Validation

- [ ] Create an agent with `--model` set and confirm the tag shows the model.
- [ ] Create an agent without a model flag and confirm the tag falls back to the
  provider label.
