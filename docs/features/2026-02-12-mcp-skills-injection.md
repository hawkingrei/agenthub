---
title: MCP and Skills Injection for ACP Sessions
date: 2026-02-12
status: implemented
---

## Summary

Support AgentHub-level MCP and skills injection for all ACP agents by loading
`~/.agenthub/mcp.json` and `~/.agenthub/skills.json`, then injecting MCP servers
into ACP session creation and codex-style `<skill>` blocks into every prompt.

## Background

We need a config-driven way to attach MCP servers and skill instructions to ACP
sessions without relying on per-agent CLI arguments. This must work uniformly
across Codex, Gemini, and Kimi ACP agents.

## Decision

- Load MCP servers from `~/.agenthub/mcp.json` using the `mcpServers` map and
  inject them into `new_session` and `load_session` requests.
- Respect the agent's MCP capabilities: filter out unsupported transports (HTTP
  or SSE) based on the `initialize` response.
- Load skills from `~/.agenthub/skills.json`, allow arbitrary `SKILL.md` paths,
  and force inject them into every ACP prompt using codex-style `<skill>` blocks.
- Attach a `_meta.agenthub.skills` array to ACP session setup for agents that
  want to consume skill metadata directly.

## Scope

- `src/acp.rs`
- `docs/features/2026-02-12-mcp-skills-injection.md`
- `docs/todo.md`

## Validation

- [ ] Start Codex ACP with `mcp.json` configured and confirm MCP tools appear.
- [ ] Start Gemini ACP with `mcp.json` configured and confirm MCP tools appear.
- [ ] Start Kimi ACP with `mcp.json` configured and confirm MCP tools appear.
- [ ] Configure `skills.json` and confirm injected `<skill>` blocks appear in
      the prompt for Codex, Gemini, and Kimi sessions.
