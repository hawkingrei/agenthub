---
title: ACP Linkerdog Provider Defaults and Session Capabilities
date: 2026-02-23
status: implemented
---

## Summary

Extend ACP provider support with `linkerdog`, add provider-scoped default session
settings (`default_mode` and `default_model`), and publish ACP
`session_capabilities` into the AgentHub event stream so Debug Session Controls
can offer capability-driven suggestions.

## Background

AgentHub previously detected only Codex/Gemini/Kimi ACP providers and only
applied a global Codex default mode. This left three gaps:

- no first-class provider detection for `linkerdog`;
- no provider-scoped default model/mode policy;
- no structured session capability handoff (`modes`/`models`/`config_options`)
  from ACP setup responses to the web Debug controls.

## Key Decisions

- Refactor ACP provider detection in `AgentManager` into provider specs
  (command matcher + argument gate), and add `linkerdog acp` support.
- Introduce provider-scoped ACP defaults in config:
  `codex_acp.provider_defaults.<provider>.default_mode/default_model`.
- Keep backward compatibility by merging legacy `codex_acp.default_mode` and
  `codex_acp.default_model` into the Codex provider defaults when provider map
  values are absent.
- Emit a unified ACP `session_capabilities` event when `new_session` or
  `load_session` returns capability state.
- Parse `session_capabilities` in `web/src/acp.ts` and surface capability
  suggestions in Debug Session Controls via native datalist suggestions while
  preserving free-text fallback.

## Scope

- `src/agent/manager/codec.rs`
- `src/agent/manager.rs`
- `src/agent/manager/tests.rs`
- `src/config.rs`
- `src/state.rs`
- `src/app.rs`
- `src/api/agents.rs`
- `src/api/teams/tests.rs`
- `src/sse.rs`
- `crates/agenthub-acp/src/lib.rs`
- `web/src/acp.ts`
- `web/src/app.tsx`
- `web/src/agent_presets.ts`
- `web/src/components/acp_debug.tsx`
- `web/src/acp.test.ts`
- `web/src/agent_presets.test.ts`
- `web/src/acp_debug.test.tsx`
- `web/src/acp_debug.interaction.test.tsx`
- `web/src/acp_panel.test.tsx`
- `web/src/output_body.test.tsx`
- `web/src/hooks/use_acp_conversation.interaction.test.tsx`
- `docs/todo.md`

## Validation

- [x] `cargo test -p agenthub acp_provider_for_agent_requires_expected_args`
- [x] `cargo test -p agenthub acp_provider_defaults_`
- [x] `cargo test -p agenthub-acp session_capabilities_event_`
- [x] `npm --prefix web test -- acp.test.ts agent_presets.test.ts acp_debug.test.tsx acp_debug.interaction.test.tsx output_body.test.tsx acp_panel.test.tsx use_acp_conversation.interaction.test.tsx`
- [x] `npm --prefix web run lint`
- [ ] `npm --prefix web run build` (local environment missing
      `@tailwindcss/postcss` in `web/node_modules`; rerun after dependency sync)
- [ ] Manual: run Codex/Gemini/Kimi/Linkerdog ACP sessions and verify
      `session_capabilities` suggestions and provider-scoped mode/model defaults
      in Debug Session Controls.
