# Team MCP Enforcement Historical Reference

> Historical Note
>
> Team runtime coordination is now CLI-first via `agenthub actor ...`.
> Actor MCP injection is no longer the active mainline runtime contract.
> This document remains only as a compact historical reference for the
> earlier MCP-first enforcement pass.
>
> Current canonical Team coordination contracts live in:
>
> - `docs/features/actor-foundation.md`
> - `docs/features/agents-teams.md`
> - `docs/features/team-mailbox-intake-and-ownership.md`
> - `docs/features/team-workspace-memory-contract.md`
>
> In any conflict, those active specs take precedence over this historical
> reference.

## Problem

Older Team rollout work established several durable collaboration lessons, but the original
MCP-first enforcement spec had become too large and too close to a deprecated runtime shape. Keeping
the full historical algorithm beside the active CLI-first specs risked duplicated or conflicting
norms.

## Scope

- Preserve the durable lessons from the historical MCP-first enforcement pass.
- Map those lessons onto the active CLI-first Team specs.
- Provide one compact background page for older rollout journals and PR archaeology.

## Non-Goals

- Defining the current Team startup or turn-loop contract.
- Reintroducing mailbox MCP as the primary Team runtime transport.
- Carrying new normative Team behavior only in a historical reference.

## Architecture

### 1) Durable Lessons From The Historical MCP Pass

The original Team MCP enforcement work contributed four lessons that still matter after the runtime
transport changed:

1. runtime capability checks should be authoritative;
2. collaboration should follow one canonical transport instead of ad-hoc bypasses;
3. inbox/ownership/reply discipline should be explicit in prompts and skills;
4. compact prompt tails and pointer-first memory improve long-lived coordination reliability.

### 2) Current Canonical Mapping

Those lessons now live in active specs instead of this historical page:

| Historical theme | Current canonical spec |
|---|---|
| actor identity, delivery semantics, CLI-first transport | `docs/features/actor-foundation.md` |
| Team wakeup, reply discipline, task-first coordination | `docs/features/agents-teams.md` |
| mailbox triage, ownership, reply-required routing | `docs/features/team-mailbox-intake-and-ownership.md` |
| workspace memory, recovery spine, pointer-first continuity | `docs/features/team-workspace-memory-contract.md` |

### 3) What Was Retired

The following ideas are historical and not the active Team contract anymore:

- mailbox MCP required startup gates;
- MCP-specific `actor_inbox` / `actor_ack` / `actor_send` capability enforcement;
- MCP-only "no shell bypass" wording as the primary runtime policy shape;
- MCP-specific allowed-action blocks and turn algorithms.

## Contracts

- This document is informational background only.
- When older journals or PRs mention MCP-first Team enforcement, read them through the canonical
  mapping above rather than treating the old MCP wording as current runtime truth.
- New Team behavior changes must update the active specs, not this historical reference.

## Validation Matrix

- Ensure the active Team specs absorb the durable enforcement lessons before shrinking this document.
- Ensure historical rollout journals still have one stable document to point at when they reference
  the older MCP-first phase.
- Keep the canonical mapping links above valid whenever Team spec paths change.

## Operational Notes

- Use this page only when reading older rollout material or explaining why current Team docs use
  CLI-first mailbox wording.
- Do not expand this page with new live contracts; update the active specs and, if needed, add one
  short historical note here instead.

## Open Risks

- Older journals and comments may still use MCP-first wording, which can confuse new contributors if
  the canonical mapping is not explicit.
- Future documentation work may accidentally duplicate active Team rules here unless historical pages
  stay compact and clearly marked.

## Source Journals

- `docs/journal/2026-03-05-team-mcp-enforcement-lessons-from-slock.md`
- `docs/journal/2026-05-21-team-spec-refresh-from-daemon-review.md`
