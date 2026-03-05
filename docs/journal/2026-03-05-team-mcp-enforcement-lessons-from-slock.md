# 2026-03-05 Team MCP Enforcement Lessons From Slock

## Context

Team collaboration already has role/phase/mailbox contracts, but runtime enforcement was still weaker
than policy text in some paths. We reviewed public Slock daemon/runtime implementation to extract
practical MCP-first workflow constraints.

## What We Learned

Slock's practical enforcement pattern is layered:

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
    - design principles and Slock-to-AgentHub mapping table
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

## External References

- `https://slock.ai`
- `https://unpkg.com/@slock-ai/daemon@0.7.0/dist/index.js`
- `https://unpkg.com/@slock-ai/daemon@0.7.0/dist/chat-bridge.js`
- `https://www.npmjs.com/package/@slock-ai/daemon`
