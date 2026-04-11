# Team Runtime Context Layout Bootstrap

## Summary

Bootstrapped the Team runtime workspace `.cache/context` layout during agent startup so leader and
worker sessions both begin with the same filesystem-backed context skeleton instead of creating
ad-hoc paths only when specific artifact writes happen later.

## Changes

- Replaced the leader-only workdir helper with a Team runtime workspace initializer in
  `src/agent/manager.rs`.
- The startup path now initializes the Team workspace context layout from
  `src/agent/manager/session.rs`.
- The initializer:
  - still creates the leader coordination workdir when it does not exist yet;
  - creates `.cache/context/run/` and `.cache/context/memory/`;
  - creates empty index files:
    - `state.md`
    - `decisions.md`
    - `errors.md`
    - `log.md`
    - `memory/profile.md`
    - `memory/project_facts.md`
    - `memory/decision_journal.md`
    - `memory/open_questions.md`
- Added focused unit coverage for:
  - leader workspace bootstrap;
  - worker workspace bootstrap in an existing workdir;
  - non-Team contexts remaining no-op;
  - invalid leader path error reporting.
- Added TeamManager-side runtime workspace resolution so continuity artifacts and memory-flush
  artifacts are written under the derived Team runtime workspace, not the base persisted agent
  config workdir.
- Added a leader-path regression test to confirm oversized continuity artifacts land under the
  derived leader coordination workspace.
- Centralized Team actor-context construction and member-role lookup into shared Team helpers so
  runtime startup and TeamManager artifact resolution stop re-encoding the same role/context rules
  in multiple places.

## Why

This is a small backend step toward the workspace-scoped context isolation contract in
`docs/features/team-workspace-memory-contract.md`:

- Team members should write into their own workspace-local `.cache/context` tree.
- Runtime-owned context paths should be predictable before continuity offload or memory flush
  features attempt to persist artifacts.
- The startup path is the right place to establish the contract because it is where the final
  runtime workdir is already resolved for leader and worker roles.

## Validation

- Ran `cargo test -q ensure_team_runtime_workspace_layout --lib`
- Result: `4 passed; 0 failed`
- Ran `cargo test -q complete_step_offloads_large_output_to_workspace_context_artifact --lib`
- Result: `1 passed; 0 failed`
- Ran `cargo test -q complete_step_offloads_large_output_to_leader_runtime_workspace_context_artifact --lib`
- Result: `1 passed; 0 failed`
- Audited `src/team` and `src/agent` filesystem write sites for Team runtime-owned `.cache/context`
  writes; no additional obvious backend paths were found still targeting the base persisted
  `agents.workdir`.

## Follow-up

- This does not yet enforce or migrate every context write in the runtime onto the new stable index
  files.
- `AGENTS.md`, workspace-root `TODO.md`, and worker `.agenthubmemory/` remain higher-level runtime
  or agent responsibilities and were intentionally left unchanged in this patch.
