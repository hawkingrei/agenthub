# 2026-03-05 Main/Node Terminology Alignment And Doc Pruning

## Context

Team mailbox and actor foundation docs still mixed old topology wording (`follow`, `local/distributed` mode wording in historical notes), while runtime now treats AgentHub as the `main` node and non-main peers as `node`.

This caused avoidable ambiguity between:

- transport layer terms (`local`/`remote`), and
- topology identity terms (`main`/`node`).

## Changes

1. Updated canonical actor spec terminology:
   - `docs/features/actor-foundation.md`
   - topology now described as `main` and `node`
   - explicit peer identity policy added:
     - `from_peer_id = main` for Team mailbox send API
     - `to_peer_id = main` for local transport
     - `to_peer_id != main` for remote transport (default `node`)

2. Updated Team domain spec terminology:
   - `docs/features/agents-teams.md`
   - added canonical `peer_id` terminology:
     - `main`: AgentHub main node
     - `node`: non-main execution node

3. Updated legacy references to canonical specs:
   - replaced outdated journal links in `docs/todo.md` with `docs/features/actor-foundation.md`
   - updated operating-model canonical references in
     `docs/journal/2026-02-24-team-operating-model-spec.md`

4. Removed outdated/superseded journal files:
   - `docs/journal/2026-02-17-team-skills-bootstrap-script.md`
   - `docs/journal/2026-02-18-agent-actor-local-distributed-architecture.md`
   - `docs/journal/2026-02-18-team-deliberation-rules-skill.md`
   - `docs/journal/2026-02-19-team-role-skill-acp-auto-injection.md`
   - `docs/journal/2026-02-20-team-single-node-skill-bootstrap.md`
   - `docs/journal/2026-02-22-team-role-skill-single-mode-isolation.md`
   - `docs/journal/2026-02-23-team-cold-start-skill-and-ui-playbook.md`
   - `docs/journal/2026-02-24-actor-agent-id-alias.md`

## Validation Notes

- Documentation-only update.
- Performed reference sweep on `docs/` and Team feature specs to remove stale links to deleted files.
- Confirmed canonical contracts now use `main/node` topology terms while transport terms remain `local/remote`.
