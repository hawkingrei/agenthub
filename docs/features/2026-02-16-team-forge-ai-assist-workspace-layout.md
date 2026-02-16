# Team Forge AI Assist + Workspace Layout

## Summary

Add two Team Forge capabilities:

- `POST /api/teams/assist` generates leader/worker prompt + skill recommendations from a short mission brief.
- Team creation supports `spec.workspace.root`; when provided, backend provisions:
  - `<root>/.agent` prompt files per member
  - `<root>/worktrees/<member>` isolated workdirs per member
  - agent `workdir` rebinding for selected team members.

## Background

The existing Team Forge flow already supports structured leader/worker config, but prompt authoring
was fully manual and workspace isolation depended on preconfigured agent paths.

For practical multi-agent onboarding, we need:

- fast prompt bootstrap from minimal user intent;
- deterministic, team-scoped filesystem layout that keeps member prompts and workdirs separated.

## Scope

- `src/api/teams.rs`
- `src/agent/manager.rs`
- `src/api/openapi.rs`
- `web/src/api.ts`
- `web/src/pages/team_page.tsx`
- `src/api/teams/tests_core.rs`
- `src/api/teams/tests_router.rs`
- `docs/todo.md`

## Key Decisions

1. Introduce `POST /api/teams/assist` with deterministic recommendation logic.
   - Input: short `brief` (+ optional member ids).
   - Output: `leader_prompt`, `leader_skills`, `worker_prompt`, `worker_skills`, `summary`.
2. Keep Team Forge UX simple:
   - Stage 1 accepts mission brief and workspace root.
   - `Generate prompts & skills` applies suggestions directly to form state.
3. Make workspace provisioning explicit and spec-driven via `spec.workspace.root`.
   - Team create preflights each member workdir binding via AgentManager safe-path checks.
   - Create prompt/workdir directories and write prompt files before persisting team definition.
   - Persist workspace metadata and per-member `prompt_file` / `workdir` into `spec`.
4. Reuse existing agent lifecycle:
   - selected member agents are not recreated;
   - only `workdir` is rebound for isolation, with running-agent guard.

## Validation

```bash
cargo test teams_api_assist_generates_prompt_and_skill_recommendations -- --nocapture
cargo test teams_api_workspace_layout_creates_prompt_files_and_isolated_workdirs -- --nocapture
cargo test openapi_json_contains_team_runs_list_path -- --nocapture
```

## Follow-ups

- Consider replacing rule-based recommendation with model-backed generation behind a feature flag.
- Add web unit/e2e tests for Team Forge assist interaction and workspace root preview UX.
