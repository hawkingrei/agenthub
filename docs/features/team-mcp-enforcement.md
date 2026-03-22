# Team MCP Enforcement Specification

> Note
>
> Team runtime coordination is now moving to a CLI-first model via
> `AGENTHUB_ACTOR_CLI` (`agenthub actor ...`) rather than mailbox MCP tool
> injection. This document is retained as the historical enforcement/design
> record; where it says "mailbox MCP required", read that as the canonical
> Team coordination capability requirement for the current runtime.

## Problem

Team collaboration quality depends on deterministic mailbox usage.
If agents can bypass MCP mailbox tools, routing and replay contracts become probabilistic:

- messages may skip `run_id` partitioning;
- ack/evidence chain may be incomplete;
- leader/worker workflow can drift from Team contracts.

We need a stronger enforcement model so Team sessions stay on the canonical coordination path by default, not best effort.

## Scope

- Team-session coordination enforcement model (`required`, fail-fast, fallback policy).
- Startup and turn-loop workflow contract for leader/worker agents.
- Allowed-action policy for mailbox communication.
- Observability and validation requirements for enforcement rollout.

## Non-Goals

- Replacing Team run/step orchestration model.
- Rewriting provider-specific SDK/runtime internals.
- Defining full multi-tenant authorization policy.

## Architecture

### 1) Design Principles

1. Runtime policy is authoritative.
- Prompt text and skills are guardrails, not the root of trust.
- Team startup must fail if required mailbox MCP capability is not present.

2. Team mode has no communication bypass path.
- No shell fallback (`agenthub actor ...`) for Team collaboration messages.
- No silent downgrade from MCP mailbox to ad-hoc text routing.

3. Keep tool surface stable; gate with policy.
- Prefer an appended `Allowed actions` block over dynamic tool-schema mutation.
- Keep provider tool definitions stable across turns for deterministic behavior.

4. Keep prompt prefix stable and move volatility to dynamic tail.
- Stable: role charter, tool schemas, safety invariants.
- Dynamic: goal, next action, allowed actions, compact state, evidence pointers, error notes.

5. Keep context append-only and recoverable.
- Large observations are offloaded to `.cache/context/run/<run_id>/...`.
- Prompt includes only summary plus pointer.

### 2) External Lessons (Slock)

Slock public daemon/runtime implementation shows a practical pattern:

1. hard config gate
- For Codex runtime they pass MCP config with `enabled=true` and `required=true`.
- This prevents agent startup without the chat MCP bridge.

2. strict system-prompt contract
- Explicitly forbids direct text output and shell-based messaging bypass.
- Forces receive/send loop through MCP tools only.

3. fixed collaboration loop
- startup/read memory -> `receive_message(block=true)` -> process -> `send_message` -> `receive_message(block=true)`.

4. minimal communication toolset
- `receive_message`, `send_message`, `read_history`, `list_server`.

Takeaway:
- hard enforcement is config/runtime;
- prompt rules are behavioral guardrails;
- loop contract keeps collaboration deterministic.

### 3) Slock Pattern To AgentHub Mapping

| Slock pattern | AgentHub equivalent | Enforcement layer |
|---|---|---|
| `required=true` communication bridge | Team mailbox MCP required at startup | Runtime hard gate |
| strict receive/send prompt contract | Team role skill protocol + allowed-action block | Prompt/skill gate |
| fixed receive -> process -> send loop | inbox -> process -> ack -> send/report loop | Runtime + skill |
| minimal collaboration tools | `actor_inbox`, `actor_ack`, `actor_send` as primary mailbox tools | Capability/profile gate |

### 4) AgentHub Enforcement Model (v1)

#### 1) Layered Enforcement

Layer A: Runtime hard gate (authoritative)

- Team actor sessions must have mailbox MCP available.
- If Team role is attached and mailbox MCP is missing, session bootstrap should fail-fast.
- No silent downgrade to shell-based actor CLI in Team mode.
- Team mode should also verify required mailbox tool names are available:
  - `actor_inbox`
  - `actor_ack`
  - `actor_send`

Layer B: Prompt/skill gate (behavioral)

- Team runtime skill must require inbox-first turn behavior:
  - first mailbox action each turn is `actor_inbox`;
  - process/ack pending messages before final response.
- Shell-based mailbox bypass (`agenthub actor ...`) is disallowed in Team sessions.

Layer C: Profile gate (context budget + precision)

- Default role skill set stays minimal.
- `team-deliberation-rules` is optional and loaded only when present in member skills profile.

## Contracts

### 1) Capability Contract (runtime-facing)

Canonical Team policy contract (implementation target):

```json
{
  "team": {
    "mcp_policy": {
      "mailbox_required": true,
      "required_mailbox_tools": [
        "actor_inbox",
        "actor_ack",
        "actor_send"
      ],
      "allow_shell_mailbox_bypass": false,
      "non_team_mailbox_required": false
    }
  }
}
```

Policy semantics:

- `mailbox_required=true`: Team role startup must fail when mailbox MCP capability is absent.
- `required_mailbox_tools`: startup fails if any required mailbox tool is missing.
- `allow_shell_mailbox_bypass=false`: Team prompts/skills/runtime policy must reject shell-based message transport.
- `non_team_mailbox_required=false`: non-Team sessions keep optional behavior.

### 2) Session Startup Contract

For Team role sessions:

1. attach role skills from role profile (`leader|worker`).
2. attach actor runtime skill.
3. verify mailbox MCP server exists in effective MCP list.
4. if check fails:
   - fail session start with explicit error;
   - append Team run event with failure reason.

For non-Team sessions:

- mailbox MCP remains optional.

### 2.1) Startup Algorithm (reference)

```text
Input: session request, role profile, effective MCP config, effective skills
If role in {leader, worker} and session_mode == Team:
  1) Load shared + role skill profile.
  2) Resolve effective MCP servers/capabilities.
  3) Verify mailbox MCP server exists.
  4) Verify required mailbox tools exist.
  5) If any check fails:
       - emit enforcement-failure run event
       - return structured startup error
       - do not start provider runtime loop
Else:
  Continue with non-Team startup policy.
```

Startup must produce one deterministic result:

- `started`: all Team enforcement checks passed.
- `failed`: explicit policy violation with structured reason.

### 3) Turn-Loop Contract

Required loop:

1. pull: `actor_inbox`.
2. process: resolve pending messages.
3. ack: `actor_ack` each consumed message exactly once.
4. dispatch/report: `actor_send` if needed.
5. wait next turn.

Policy:

- no direct output as replacement for mailbox reply in Team workflow;
- no shell fallback for messaging when MCP tools are available.
- each consumed message should have exactly one ack result state (`ok` or `failed`);
- when processing fails, ack should still include a compact failure reason pointer.

### 3.1) Turn Algorithm (reference)

```text
Before turn:
  - load compact state + todo + recent error pointers
  - enforce Allowed actions gate
Turn:
  1) msgs = actor_inbox(run_id, actor_id)
  2) for msg in msgs:
       process(msg)
       actor_ack(run_id, actor_id, message_id, result)
       if outbound needed: actor_send(run_id, from_actor_id, to_actor_id, payload)
After turn:
  - append decision/error/log entries
  - persist large artifacts to .cache/context/run/<run_id>/...
  - set next action in todo
```

### 4) Allowed-Action Policy

Team mode should append an explicit allowed-action block:

- mailbox actions: `actor_inbox`, `actor_ack`, `actor_send`.
- optional read-only context/tool actions for execution tasks.
- denylist for communication bypass paths in Team mode.

This block should be policy text, not provider-specific tool-schema mutation.

### 4.1) Allowed-Action Block Template

```text
[Allowed actions - Team mode]
allow:
- actor_inbox
- actor_ack
- actor_send
- <task-scoped read-only tools>
deny:
- shell mailbox commands (agenthub actor send/inbox/ack)
- direct conversation bypass when mailbox route is required
```

Placement:

- keep this block in dynamic tail near `Goal` and `Next action`;
- do not inject volatile values into stable prefix sections.

### 5) Failure Policy

Bootstrap failures:

- mailbox MCP missing;
- role-skill injection mismatch;
- malformed role profile.

Handling:

- fail-fast, structured error, visible run event.
- no hidden fallback that can violate Team routing contracts.

### 5.1) Enforcement Error Codes

| Code | Meaning | Operator action |
|---|---|---|
| `TEAM_MCP_MISSING` | Team role started without mailbox MCP capability | Fix MCP config and restart Team session |
| `TEAM_MCP_TOOL_MISSING` | mailbox MCP loaded but required tools are incomplete | Align MCP server/tool version |
| `TEAM_ROLE_PROFILE_INVALID` | role skill profile malformed or inconsistent | Fix role profile and rerun bootstrap |
| `TEAM_ALLOWED_ACTIONS_VIOLATION` | turn attempted disallowed communication path | inspect prompt/skill and runtime policy |

Error surface contract:

- API returns structured code + short message.
- Team run event stores code, role, and capability snapshot pointer.
- Debug panel can inspect last enforcement failure per member.

### 6) Observability Requirements

Run events (minimum):

- `team_enforcement_check_started`
- `team_enforcement_check_passed`
- `team_enforcement_check_failed`
- `team_turn_policy_violation`

Metrics (minimum):

- `team_enforcement_start_fail_total{reason,role}`
- `team_turn_inbox_first_violation_total{role}`
- `team_mailbox_ack_missing_total{role}`
- `team_mailbox_send_total{role,channel}`

Debug snapshot fields:

- effective role profile;
- effective MCP servers and required tool presence;
- allowed-actions block hash (for drift detection);
- last policy violation pointer.

### 7) Prompt Assembly Contract (Team Mode)

Stable prefix:

- role charter and safety invariants;
- tool schemas and static formatting blocks.

Dynamic tail:

- current goal;
- next action;
- allowed actions block;
- compact state summary;
- evidence pointers;
- recent error notes.

Context memory expectations:

- append-only `decisions`, `errors`, `log`;
- oversized artifacts in filesystem memory with pointers;
- pre-compaction flush outcome tracked as `persisted` or `noop`.

## Rollout Plan

### Phase 1 (done/partially done baseline)

- role-minimal skill profile established.
- deliberation skill switched to explicit-profile opt-in.
- runtime skill includes inbox-first protocol language.

### Phase 2 (enforcement hardening)

- implement Team-session mailbox MCP required check with hard failure.
- add explicit run event type for enforcement failures.
- add API/debug surface to inspect effective Team runtime capabilities.

### Phase 3 (policy hardening)

- add Team-mode bypass detection telemetry.
- add optional strict mode to reject turns violating inbox-first contract.

## Validation Matrix

1. Runtime startup checks
- Team leader/worker session fails if mailbox MCP missing.
- Team leader/worker session fails if required mailbox tool is missing.
- non-Team session still starts without mailbox MCP.

2. Workflow checks
- Team turn processes inbox before final output in representative scenarios.
- ack/evidence chain preserved across retries and partial failures.
- no shell bypass in Team mode.

3. Context checks
- role-minimal skills loaded by default.
- deliberation skill loaded only when explicitly declared.
- dynamic tail contains allowed-actions and next action each turn.

4. Observability checks
- capability logs include effective MCP/skills snapshot.
- enforcement failure events visible in Team run events/debug panel.
- metrics counters increase on synthetic policy violations.

5. Suggested test targets
- `cargo test -p agenthub -- team::manager::tests`
- `cargo test teams_router_http_contract -- --nocapture`
- `cargo test -p agenthub-team-actor`
- add Team enforcement negative-path API tests for startup and turn policy violations.

## Open Risks

- Provider-specific runtime behaviors may differ in strictness.
- Hard-fail policy can increase bootstrap failure rate if operator config is incomplete.
- Strict inbox-first enforcement needs careful exception handling for emergency/system responses.
- Debug/event payload growth needs bounded retention to avoid noisy storage.

## Operational Notes

- Keep this spec aligned with:
  - `docs/features/teams-collaboration-playbook.md`
  - `docs/features/agents-teams.md`
  - `docs/features/actor-foundation.md`
- Prefer runtime capability checks over prompt-only enforcement when conflicts appear.
- Keep Team role defaults minimal and add optional skills only by explicit profile.

## Source Journals

- `docs/journal/2026-03-05-team-mcp-enforcement-lessons-from-slock.md`
- `docs/journal/2026-02-24-team-operating-model-spec.md`
- `docs/journal/2026-02-18-acp-actor-mailbox-native-tools.md`

## References

- `https://slock.ai`
- `https://unpkg.com/@slock-ai/daemon@0.7.0/dist/index.js`
- `https://unpkg.com/@slock-ai/daemon@0.7.0/dist/chat-bridge.js`
- `https://www.npmjs.com/package/@slock-ai/daemon`
