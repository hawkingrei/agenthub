# Agent Runtime Profiles

## Problem

AgentHub agents need explicit runtime profile settings so operators can choose the
provider adapter, model, and thinking level per agent instead of relying on global
CLI defaults or provider-local configuration files. This is especially important
for Team work, where coordinator and worker agents may need different model
strengths and reasoning budgets.

The first supported profile targets are Codex and Claude Code through the ACP
runtime boundary.

## Scope

- Persist per-agent runtime profile intent for ACP-backed agents.
- Support Codex and Claude Code as first-class profile targets.
- Let operators set a model label per agent.
- Let operators set a provider-neutral thinking level per agent.
- Preserve current startup behavior for agents without an explicit profile.
- Expose the profile in web create/edit flows and runtime summaries.

## Non-Goals

- AgentHub does not own provider credentials in this spec.
- AgentHub does not maintain a live provider model catalog in this spec.
- AgentHub does not expose raw provider-specific option bags as the primary UI.
- This spec does not replace the ACP control/event boundary.
- This spec does not add non-ACP local model orchestration.

## Architecture

Agent runtime profile configuration should be stored as agent metadata owned by
AgentHub and translated at launch time by the selected adapter.

The canonical profile shape is provider-neutral:

- `provider`: the runtime adapter family, initially `codex` or `claude_code`
- `model`: an operator-provided model identifier string, optional when the
  provider default should apply
- `thinking_level`: an optional reasoning budget enum, initially `low`,
  `medium`, `high`, or `max`

Provider adapters translate the neutral profile into their native startup
configuration:

- Codex maps `model` and `thinking_level` to the Codex ACP adapter startup
  options supported by `agenthub-acp codex`.
- Claude Code maps `model` and `thinking_level` to the Claude ACP adapter
  startup options supported by `agenthub-acp claude` / Claude Code settings.

The main AgentHub backend should validate and persist the neutral profile, but it
should not encode provider-specific credential lookup or session internals.

## Contracts

- Each agent may have zero or one runtime profile.
- Missing profile means compatibility mode: keep the current adapter default
  behavior.
- `provider` is required when a profile exists.
- `model` is a non-empty trimmed string when set; unknown model names are allowed
  because provider availability can drift independently from AgentHub releases.
- `thinking_level` is a constrained enum, not a free-form string.
- Runtime profile updates affect future launches and newly created sessions.
  They do not mutate an already-running provider session in place.
- Web create/edit surfaces must show the effective provider, model, and thinking
  level without exposing secrets.
- Team member creation should allow different profiles for coordinator and worker
  agents.
- AgentHub should pass only explicit profile fields to adapters. If a field is
  unset, the adapter/provider default remains authoritative.
- Adapter diagnostics may report the effective provider/model/thinking level, but
  diagnostics must not become the source of truth for persisted configuration.

## Validation Matrix

- Backend persistence tests:
  - create an agent with no runtime profile and preserve compatibility defaults
  - create/update an agent with `codex`, model, and thinking level
  - create/update an agent with `claude_code`, model, and thinking level
  - reject invalid provider values and invalid thinking levels
- Adapter launch tests:
  - Codex launch receives explicit model and thinking level when configured
  - Claude Code launch receives explicit model and thinking level when configured
  - unset fields are omitted so provider defaults still apply
- Web tests:
  - create/edit forms render provider, model, and thinking level controls
  - Team member creation can set different profiles per member
  - runtime summary displays the effective profile compactly
- Compatibility checks:
  - existing agents without runtime profile continue to launch
  - existing `codex_acp_default_mode` behavior remains independent from model and
    thinking level

## Operational Notes

- Model names and provider capabilities can change faster than AgentHub releases,
  so the UI should not hard-code a closed model catalog as a correctness gate.
- Thinking level is intentionally provider-neutral. Adapters are responsible for
  mapping unsupported values to the nearest supported provider behavior or
  returning a clear launch error.
- Profile settings should be visible in diagnostics because model mismatches are
  a common source of operator confusion.
- Credential setup remains provider-owned or adapter-owned. AgentHub should only
  reference the provider family and launch intent.

## Open Risks

- Codex and Claude Code may not expose identical reasoning-budget semantics, so
  `thinking_level` mapping needs careful adapter documentation.
- Provider model aliases may drift or disappear, causing launch failures after a
  profile previously worked.
- Team templates may need default profile presets so operators do not repeatedly
  configure every worker by hand.
- Remote node execution must receive the same profile intent without leaking
  credentials or assuming identical local provider config on every node.

## Source Journals

- [2026-02-09 Codex ACP Protocol Sync](../journal/2026-02-09-codex-acp-protocol-sync.md)
- [2026-06-10 Claude ACP Provider Support](../journal/2026-06-10-claude-acp-provider-support.md)
- [2026-06-11 Generic Codex ACP Entrypoint](../journal/2026-06-11-generic-codex-acp-entrypoint.md)
