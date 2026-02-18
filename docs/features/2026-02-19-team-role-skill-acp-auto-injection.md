# Team Role Skill ACP Auto-Injection

## Background

Team role skills were configured in Team spec and `skills.json`, but Team actor sessions still
depended on external bootstrap setup to guarantee ACP prompt injection. This created avoidable
operator friction.

## Scope

- `crates/agenthub-acp/src/lib.rs`
- `crates/agenthub-acp/src/team_role_skills.rs`
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
- `src/team/orchestrator.rs`
- `src/actor_runtime.rs`
- `src/api/agents.rs`

## Key Decisions

1. Extend actor runtime context with optional `member_role`.
2. Team orchestrator resolves `spec.members[].role` for each dispatched step and passes it into
   ACP actor context.
3. ACP runtime auto-injects built-in Team role skills by actor role:
   - `leader`: `team-leader-orchestrator` + `team-deliberation-rules`
   - `worker`: `team-worker-executor` + `team-deliberation-rules`
4. Keep `agenthub-actor-runtime` built-in injection and de-duplicate skills by name/path to avoid
   duplicate prompt blocks when `skills.json` already contains same skill entries.

## Validation

Executed:

```bash
cargo fmt
cargo test -p agenthub-acp -- --nocapture
cargo test dispatch_once_injects_actor_runtime_and_supports_inbox_ack_flow -- --nocapture
cargo test parse_member_role_returns_expected_role -- --nocapture
cargo test teams_api_ -- --nocapture
npm --prefix web run test -- src/pages/team_page.runs.test.ts
```

Observed:

- ACP crate tests passed, including new Team role skill injection tests.
- Team orchestrator dispatch test passed with role propagation in actor context.
- Team API tests passed.
- Team page helper tests stayed green.
