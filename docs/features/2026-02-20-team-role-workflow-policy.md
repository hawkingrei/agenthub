# 2026-02-20 Team Role Workflow Policy Defaults

## Background

Team runs needed a clearer role boundary between leader and worker agents:

- leader should focus on architecture/review/orchestration instead of direct feature coding
- worker should execute in isolated git worktrees with random branches and regular `main` sync

Without explicit defaults, these constraints were easy to drift across backend prompt injection and Team UI draft defaults.

## Scope

- Update backend default Team prompts (`src/api/teams.rs`) for leader and worker role policy.
- Update frontend Team draft prompt defaults (`web/src/pages/team/member_helpers.ts`) to match backend semantics.
- Enforce role workflow policy at runtime start in backend `AgentManager` (`src/agent/manager.rs`).
- Add regression assertions in Rust/Web tests so role-policy prompt guarantees remain stable.
- Record policy in project charter (`AGENTS.md`) under requirement additions.

## Key Decisions

1. **Leader no-code guardrail in default prompt**
   - Leader is explicitly defined as architect/reviewer/efficiency owner.
   - Leader explicitly owns technical research and option comparison before delegation.
   - Leader should not implement feature code directly.
   - Leader is guided to start from empty workspace and maintain `AGENTS.md`.
   - Leader review flow should prefer GitHub CLI (`gh`) or explicit clone-only review workspaces.

2. **Worker isolation + sync guardrail in default prompt**
   - Worker must use its own git worktree (no shared worktree across workers).
   - Worker creates a random branch per task/session.
   - Worker periodically syncs from `main` and reports conflicts promptly.
   - Worker may coordinate with peers for dependencies, but must report status/evidence to leader.

3. **Prompt parity across backend and frontend**
   - Keep the same role policy text in backend spec normalization and UI draft defaults.
   - Prevent backend/frontend prompt drift during future Team UI/API changes.

4. **Runtime start policy is the final gate**
   - Leader startup now rejects non-`use_existing` mode and rejects non-empty workdir.
   - Worker startup now requires `create_worktree` + `worktree_repo`, rewrites runtime workdir to per-run isolation, and creates a random worker branch.
   - Runtime gate is applied in `start_agent_with_actor_context`, so policy is enforced even if prompts are edited manually.

## Validation

- `cargo test teams_api_injects_role_workflow_prompt_policy_defaults -- --nocapture`
- `cargo test runtime_start_policy_ -- --nocapture`
- `npm --prefix web run test -- src/pages/team/state.test.ts src/pages/team/create_helpers.test.ts`
