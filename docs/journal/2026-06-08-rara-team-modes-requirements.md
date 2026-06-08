# Rara Team Modes Requirements

## Summary

Captured the next Rara direct integration requirements for local Rara-managed teams and remote
AgentHub Team member execution.

The durable contract now distinguishes the outer AgentHub Team member identity from an optional
Rara-managed nested subteam.

## Background

Rara is a first-party runtime rather than an ACP-only adapter. AgentHub therefore needs to support
Rara's own team behavior while preserving AgentHub's canonical Team roles, mailbox routing, task
ownership, permission review, and agent-card semantics.

The new requirement is not only "run Rara remotely." It also needs Rara to decide whether a remote
Team message or task matches the assigned AgentHub agent card before acting.

## Scope

- Local Rara agent teams where a lightweight Rara leader may coordinate Rara subagent workers.
- Remote AgentHub Team members where AgentHub assigns the Rara-backed agent a `coordinator` or
  `worker` role.
- Rara-managed nested subteams under the outer AgentHub Team member identity.
- Lightweight semantic guard outcomes for agent-card and role compatibility.

## Key Decisions

- AgentHub observes a local Rara agent team as one outer AgentHub runtime unless Rara exposes safe
  structured subagent telemetry.
- A remote Rara-backed AgentHub Team member keeps exactly one AgentHub member/actor identity even if
  Rara starts its own internal local or remote subagents.
- Rara internal subagent ids do not become AgentHub Team member ids and must not bypass AgentHub
  mailbox routing.
- AgentHub startup context for Team-backed Rara must include safe Team identity and agent-card
  fields.
- Rara may run a lightweight semantic judge with three outcomes:
  - `compatible`
  - `mismatch`
  - `needs_clarification`
- A guard mismatch is a safe runtime status, not a crash, cancel, or permission denial.
- Agent-card updates must flow through AgentHub profile patch proposals instead of out-of-band Rara
  mutation.

## Validation

Documentation-only checkpoint:

```bash
git diff --check
cargo fmt --check
```

Implementation validation should add focused tests for:

- local Rara team startup context
- remote AgentHub Team member startup context
- semantic guard outcome translation
- nested Rara subagent identity isolation
- mailbox and permission-review routing remaining bound to the outer AgentHub member

## Follow-Ups

- Define the concrete runtime-control request/event fields for Team identity and safe agent-card
  context.
- Define the Rara-side lite semantic guard event schema.
- Decide whether and how safe Rara subagent telemetry appears in AgentHub diagnostics without making
  those subagents first-class AgentHub Team members.
