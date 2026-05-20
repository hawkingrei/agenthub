# 2026-03-05 Team MCP Enforcement External Review

> Historical Note
>
> This journal records the original MCP-first enforcement pass.
> The active Team runtime is now CLI-first, and the durable lessons from this
> review are now captured in:
>
> - `docs/features/actor-foundation.md`
> - `docs/features/agents-teams.md`
> - `docs/features/team-mailbox-intake-and-ownership.md`
> - `docs/features/team-workspace-memory-contract.md`
> - `docs/features/team-mcp-enforcement.md` (historical reference only)
> - `docs/journal/2026-05-21-team-spec-refresh-from-external-daemon-review.md`

## Context

Team collaboration already has role/phase/mailbox contracts, but runtime enforcement was still weaker
than policy text in some paths. We reviewed a public external daemon/runtime implementation to
extract practical MCP-first workflow constraints.

## What We Learned

The reviewed external daemon pattern was layered:

1. hard runtime config
- mark communication MCP as required in runtime config so startup cannot proceed without it.

2. strict prompt contract
- prohibit shell/file-based communication bypass for agent collaboration.

3. deterministic receive/send loop
- receive -> process -> send -> receive, with persistent memory reload after wake.

4. minimal communication toolset
- expose only small collaboration primitives (`receive`, `send`, `history`, `member discovery`).

## Documentation Changes

- added detailed Team MCP enforcement spec:
  - `docs/features/team-mcp-enforcement.md`
  - expanded with:
    - design principles and external-pattern-to-AgentHub mapping table
    - runtime capability contract (`mailbox_required`, required mailbox tools, bypass policy)
    - startup and turn-loop reference algorithms
    - allowed-actions block template for Team mode
    - enforcement error code table and operator remediation actions
    - observability contract (run events, metrics, debug snapshot fields)
    - detailed validation matrix and suggested test targets
- linked enforcement profile into existing Team/Actor feature docs:
  - `docs/features/teams-collaboration-playbook.md`
  - `docs/features/agents-teams.md`
  - `docs/features/actor-foundation.md`

## Result

Team docs now include an explicit MCP-first enforcement model with:

- runtime fail-fast expectations for missing mailbox MCP in Team mode;
- inbox-first turn loop contract;
- allowed-action policy guidance;
- rollout/validation matrix for staged hardening.

## Current Status

The MCP-first contract recorded here is no longer the active Team runtime path. Its durable lessons
were compacted into the active Team specs, while the large MCP-era spec was reduced to a short
historical reference page.
