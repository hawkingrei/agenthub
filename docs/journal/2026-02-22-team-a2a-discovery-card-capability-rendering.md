# Team A2A Discovery Card Capability Rendering

## Background

Team roadmap phase-5 requires two connected capabilities:

- expose an A2A-style discovery card from a well-known endpoint;
- render peer capability metadata in Team UI before delegation.

Before this change, Team member capability visibility only came from snapshot role/model/skills fields, and there was no dedicated discovery-card contract.

## Scope

- Backend:
  - added `GET /api/agents/:id/.well-known/agent-card` in `src/api/agents.rs`;
  - introduced a stable discovery payload (`card_id`, `schema_version`, `description`, `identity`, `runtime`, `capability_tags`);
  - derived runtime ACP provider via existing agent command/provider detection;
  - bound `description` to team member profile (`spec.members[].description`) when available, with fallback to runtime-generated summary.
- Frontend:
  - added `AgentDiscoveryCardRecord` type and `api.getAgentDiscoveryCard(...)` in `web/src/api.ts`;
  - in `web/src/pages/team_page.tsx`, added per-member discovery-card fetch/cache on member selection;
  - in `web/src/pages/team_member_console_panel.tsx`, rendered discovery metadata (`description`, `acp_provider`, `worktree_mode`, `code_mode`, `capability_tags`) with loading/fallback states;
  - in Team create worker wizard, added `description` input and serialized it to `spec.members[].description`.
- Tests:
  - added backend unit/router coverage in `src/api/agents.rs`;
  - extended web unit/e2e fixtures for discovery-card rendering and worker-description serialization.

## Key Decisions

1. Use a member-scoped well-known path under agent resource.

- Path: `/api/agents/:id/.well-known/agent-card`.
- Keeps auth and agent ownership semantics aligned with existing `/api/agents/:id/*` routes.

2. Keep discovery payload compact and runtime-oriented.

- Identity and runtime are separated.
- Capability tags are explicit and machine-readable (`team_mailbox_v1`, `team_step_execution_v1`, `acp_*`, etc.).
- Human-facing role identity is provided by team spec member description instead of generated boilerplate when configured.

3. Apply fetch-on-select with local cache in Team UI.

- Avoids loading all member cards on every run snapshot refresh.
- Preserves responsive member switching while avoiding repeated failed calls.

## Validation

Executed locally:

- `cargo test build_agent_discovery_card_includes_runtime_tags --package agenthub`
- `cargo test build_agent_discovery_card_prefers_member_description --package agenthub`
- `cargo test resolve_member_description_from_spec_matches_member_entry --package agenthub`
- `cargo test discovery_card_route_exposes_agent_capabilities --package agenthub`
- `cargo test team_run_snapshot_api_returns_member_status_and_mailbox_summary --package agenthub`
- `npm --prefix web test -- src/pages/team/create_helpers.test.ts src/pages/team/create_draft_storage.test.ts src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx`
- `npm --prefix web run build`

All commands passed.
