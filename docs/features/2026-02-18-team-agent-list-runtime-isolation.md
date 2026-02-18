# Team Agent List Runtime Isolation

## Background

Team runs reuse `AgentManager` runtime start APIs (`start_agent_with_actor_context`) to execute member steps.  
When a member agent is running inside Team orchestration, showing the same agent in `/api/agents` makes Agent mode and Team mode appear coupled and creates operator confusion.

## Scope

- Update Agent list behavior (`GET /api/agents`) to hide agents that are actively executing Team steps.
- Keep Agent storage model unchanged (`agents` table remains shared).
- Keep Team APIs and run/step lifecycle semantics unchanged.

## Key Decisions

1. Use runtime state, not static membership, to isolate views.
   - Hidden only when the agent is currently bound to an active Team step.
   - Visibility returns after the Team step leaves active states.
2. Filter criteria:
   - `team_steps.status IN ('working', 'input_required')`
   - `team_runs.status IN ('submitted', 'working', 'input_required')`
   - Join path: `team_steps.remote_task_id -> agent_sessions.id -> agent_sessions.agent_id`
3. Backward compatibility:
   - If Team tables do not exist (minimal test schemas), Agent list gracefully falls back to the legacy unfiltered behavior.

## Validation

1. Added API-level regression:
   - `list_agents_hides_team_working_member_agent` in `src/api/agents.rs`
   - Asserts Team-working member agents are hidden from `GET /api/agents`.
   - Asserts non-Team agents remain visible.
2. Re-verified on current branch:
   - `cargo test list_agents_hides_team_working_member_agent -- --nocapture` (1 passed)
3. Suggested manual verification:
   - Create two agents `A/B`.
   - Start `A` through Team run orchestration.
   - Call `GET /api/agents` and confirm `A` is absent while `B` remains.
   - End Team step and confirm `A` becomes visible again.
