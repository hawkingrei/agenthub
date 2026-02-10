---
title: ACP Gemini and Kimi CLI Support
date: 2026-02-10
status: implemented
---

## Summary

Add Gemini CLI and Kimi CLI as ACP-capable runtimes alongside Codex, with
frontend presets and backend provider-aware session tracking.

## Background

AgentHub previously assumed Codex ACP for all ACP agents, which made it
impossible to launch other ACP CLIs like Gemini or Kimi while keeping ACP
session persistence and clear-session behavior correct.

## Decision

- Add Gemini and Kimi presets in the Create Agent modal with the same command
  shapes used by toad (`gemini --experimental-acp`, `kimi acp`).
- Detect ACP provider from the agent command and store persistent sessions per
  provider.
- Apply `codex_acp.default_mode` only to the Codex provider.
- Default `acp/session/clear` to the provider inferred from the agent command
  when no provider is supplied.

## Scope

- `src/agent/manager.rs`
- `src/api/agents.rs`
- `web/src/agent_presets.ts`
- `web/src/app.tsx`
- `web/src/components/create_agent_modal.tsx`
- `docs/todo.md`

## Validation

- Install Gemini CLI and start an agent with the Gemini preset; verify ACP
  events stream and the session can be cleared.
- Install Kimi CLI and start an agent with the Kimi preset; verify ACP
  events stream and the session can be cleared.
- Start a Codex ACP agent and confirm `codex_acp.default_mode` still applies.
