# Feature Docs Compaction Wave 2 Closeout

## Summary

Feature-doc compaction wave 2 is complete for the residual Workspace, Team conversation/composer,
and ACP-heavy UI journal clusters. Stable behavior now lives in canonical feature specs, all 13
source journals retain explicit supersession pointers, and the completed documentation TODO has
been removed.

## Background

Three documentation-only compaction passes had already promoted stable UI rules into feature specs:

- [`2026-07-19-workspace-ui-compaction-wave2.md`](2026-07-19-workspace-ui-compaction-wave2.md)
- [`2026-07-19-team-conversation-composer-compaction-wave2.md`](2026-07-19-team-conversation-composer-compaction-wave2.md)
- [`2026-07-19-acp-ui-compaction-wave2.md`](2026-07-19-acp-ui-compaction-wave2.md)

The source journals also received explicit supersession sections in PR #989. The remaining gap was
bookkeeping: stale follow-ups still described completed clusters as open, canonical source lists
were incomplete, and `docs/todo.md` still carried the umbrella item.

## Scope

This closeout covers three bounded clusters:

- Workspace shell UI:
  - `2026-04-18-workspace-shell-compactness.md`
  - `2026-04-18-workspace-slock-notion-density-pass.md`
  - `2026-04-19-workspace-agent-pane-chrome-tightening.md`
  - `2026-04-19-workspace-channel-first-lens-language.md`
- Team conversation and composer UI:
  - `2026-04-03-team-channel-conversation-alignment.md`
  - `2026-04-05-team-channel-send-feedback-and-visibility.md`
  - `2026-04-05-team-conversation-selection-resilience.md`
  - `2026-04-24-team-conversation-slock-polish.md`
- ACP-heavy UI:
  - `2026-02-13-acp-ui-fold-markdown-mobile.md`
  - `2026-02-15-acp-tool-call-humanized-rendering.md`
  - `2026-02-17-acp-tool-call-group-fold-animation.md`
  - `2026-02-20-web-tailwind-ui-phase8-acp-panel-debug-shell.md`
  - `2026-02-20-web-tailwind-ui-phase9-acp-conversation-shell.md`

No runtime, route, component, or styling behavior changes in this closeout.

## Key Decisions

- `docs/features/workspace-unified-ia.md` is normative for shell density, channel-first lens
  language, embedded Agent/ACP chrome, route grammar, and migration guardrails.
- `docs/features/team-channels-threads.md` and `docs/features/frontend-design.md` are normative for
  Team conversation scope, send/idempotency behavior, visible payload filtering, message-row
  rhythm, composer language, and chat markdown.
- `docs/features/acp-runtime.md` and `docs/features/frontend-design.md` are normative for ACP
  tool-call folding/grouping, humanized payloads, markdown and terminal safety, mobile headers,
  debug controls, and semantic styling hooks.
- Dated source journals remain implementation chronology and validation evidence; they are not
  parallel normative specifications.
- Future stable UI contract changes should update the canonical feature spec first and add a dated
  journal only when rollout or validation evidence needs to be retained.
- Deployed browser and performance evidence remains tracked by its own active TODOs and is not a
  prerequisite for documentation compaction closure.

## Validation

```bash
git diff --check
rg -n "^## Supersession$" docs/journal/2026-02-13-acp-ui-fold-markdown-mobile.md docs/journal/2026-02-15-acp-tool-call-humanized-rendering.md docs/journal/2026-02-17-acp-tool-call-group-fold-animation.md docs/journal/2026-02-20-web-tailwind-ui-phase8-acp-panel-debug-shell.md docs/journal/2026-02-20-web-tailwind-ui-phase9-acp-conversation-shell.md docs/journal/2026-04-03-team-channel-conversation-alignment.md docs/journal/2026-04-05-team-channel-send-feedback-and-visibility.md docs/journal/2026-04-05-team-conversation-selection-resilience.md docs/journal/2026-04-18-workspace-shell-compactness.md docs/journal/2026-04-18-workspace-slock-notion-density-pass.md docs/journal/2026-04-19-workspace-agent-pane-chrome-tightening.md docs/journal/2026-04-19-workspace-channel-first-lens-language.md docs/journal/2026-04-24-team-conversation-slock-polish.md
rg -n "^## (Problem|Scope|Non-Goals|Architecture|Contracts|Validation Matrix|Operational Notes|Open Risks|Source Journals)$" docs/features/workspace-unified-ia.md docs/features/frontend-design.md docs/features/acp-runtime.md docs/features/team-channels-threads.md
```

## Follow-Ups

None for feature-doc compaction wave 2.
