# Codex ACP Prompt Steering

## Summary

Codex ACP now allows follow-up prompts through the AgentHub ACP runtime once permission gates are clear, so Codex app-server can steer an active turn after a permission denial or active tool-call update instead of leaving later user input queued behind the stale turn. Team-managed Codex sessions also force `full-access` mode to avoid routine permission round trips.

## Background

Codex treats a denied approval as a tool failure that is returned to the model. The expected recovery path is not a prompt cancel: the turn may continue, and later user input can be steered into the active turn. AgentHub previously marked Codex ACP as strict FIFO, so later Team/member prompts remained queued whenever an active prompt was still open, even after a permission timeout or denial had already settled.

## Scope

- Codex provider prompt delivery now uses the concurrent prompt policy.
- Gemini and Kimi ACP providers remain strict FIFO.
- The existing pending session mutation and pending permission gates still block prompt dispatch, but Codex ordinary prompts are no longer blocked by AgentHub's pending tool-call diagnostic after the permission request has settled.
- Team-managed Codex sessions override the configured default mode with `full-access`; standalone Codex sessions keep the configured default mode.
- Permission denial remains `ReviewDecision::Denied` or an empty turn-scoped permission response, not cancel/abort.

## Key Decisions

- Use the existing provider-aware prompt policy instead of adding a permission-specific bypass.
- Match the Zed `codex-acp` actor shape: submit ordinary user input to Codex immediately, track the returned submission independently, and let Codex app-server own turn steering and provider-side queuing.
- Keep AgentHub's outer gates only for active permission requests and session mutation commands where sending another prompt would race approval or mode/model/config changes.

## Validation

Focused checks:

```bash
cargo test -p agenthub acp_provider_for_agent_requires_expected_args -- --nocapture
cargo test -p agenthub-acp prompt_delivery_policy_is_provider_aware -- --nocapture
cargo fmt --all --check
```

## Follow-Ups

- Validate a live Team Codex ACP denial followed by a normal channel message and confirm the later message reaches the active Codex turn instead of remaining behind the stale prompt queue.
- Extend `agenthub doctor agent-trace` live overlay to show Codex app-server active turn steerability and queued submission counts when a prompt is stale.
