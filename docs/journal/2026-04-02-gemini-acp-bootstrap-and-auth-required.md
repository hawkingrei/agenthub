---
title: Gemini ACP Bootstrap And Auth Required Handling
date: 2026-04-02
status: implemented
---

## Summary

Align AgentHub with the current Gemini CLI ACP entrypoint (`gemini --acp`) and
make Gemini authentication failures explicit for remote-host deployments.

## Background

AgentHub's original Gemini preset and provider tests were pinned to the older
`--experimental-acp` flag. Upstream Gemini CLI now documents ACP startup under
`gemini --acp`, and its ACP bootstrap exposes authentication as an explicit
`auth_required` failure rather than a hidden transport detail.

For local IDE clients, Gemini CLI can launch an interactive browser login on
the same machine. That behavior does not translate to AgentHub's remote-host
deployment model because the browser would open on the server instead of in the
end user's session.

## Decision

- Update the Gemini preset and UI/test fixtures to use `gemini --acp`.
- Keep backend Gemini provider detection backward-compatible with both `--acp`
  and `--experimental-acp` so existing agent records continue to work.
- Treat ACP `auth_required` as an explicit bootstrap failure.
- For Gemini specifically, report a host-side pre-authentication requirement
  instead of attempting an automatic Google login flow from AgentHub.

## Scope

- `src/agent/manager/acp_provider.rs`
- `src/agent/manager/session.rs`
- `src/agent/manager/tests.rs`
- `crates/agenthub-acp/src/lib.rs`
- `web/src/agent_presets.ts`
- `web/src/agent_presets.test.ts`
- `web/src/create_agent_modal.test.tsx`
- `web/tests/e2e/team_page.e2e.ts`
- `docs/features/acp-runtime.md`
- `docs/todo.md`

## Validation

- [ ] `cargo test -p agenthub acp_provider_for_agent_requires_expected_args -- --nocapture`
- [ ] `cargo test -p agenthub-acp auth_required_error_detection_matches_protocol_error_code gemini_auth_required_message_mentions_host_side_authentication -- --nocapture`
- [ ] `cd web && npm run test -- src/agent_presets.test.ts src/create_agent_modal.test.tsx`
- [ ] `cd web && npm run lint -- src/agent_presets.ts src/create_agent_modal.test.tsx tests/e2e/team_page.e2e.ts`
- [ ] Manual AgentHub validation: Create Agent preset shows `gemini --acp`.

## Follow-Up

- Add a first-class UI flow for non-interactive Gemini authentication (for
  example API-key based ACP `authenticate`) without relying on server-side
  browser login.
