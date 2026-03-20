# Team `.agenthubmemory` contract

## Summary

Team prompts/skills previously mixed durable memory guidance with `.cache/context/todo.md`.
That blurred runtime continuity state and project-owned memory inside concrete repositories.

This change adds a Team-level contract:

- workers in concrete project workspaces should keep durable memory under `.agenthubmemory/`;
- canonical files are `.agenthubmemory/TODO.md`, `.agenthubmemory/journal/`, and
  `.agenthubmemory/note/`;
- leader normally runs in an empty coordination workspace and can skip `.agenthubmemory/`;
- `.cache/context/` remains runtime continuity state rather than the primary long-lived notebook;
- AgentHub startup should ensure both `$HOME/.gitignore_global` and Git's default user ignore file
  (`$XDG_CONFIG_HOME/git/ignore` or `~/.config/git/ignore`) contain `.agenthubmemory`.

## What Changed

- `skills/team/AGENTS.md`
  - added the shared `.agenthubmemory/` role boundary and startup guidance.
- `skills/team/TEAM_AGENTS.md`
  - added `.agenthubmemory` pointers to the runtime template.
- `skills/team/team-worker-executor.SKILL.md`
  - made `.agenthubmemory/` the durable worker project-memory root in concrete repos.
- `skills/team/team-worker-agents-index.SKILL.md`
  - added startup guidance to ensure `.agenthubmemory/{TODO.md,journal/,note/}` exists for workers.
- `skills/team/team-leader-orchestrator.SKILL.md`
  - documented that leader empty coordination workspaces usually do not need `.agenthubmemory/`.
- `skills/team/team-leader-agents-index.SKILL.md`
  - aligned leader startup expectations with the empty-workspace rule.
- `crates/agenthub-team-prompts/src/lib.rs`
  - injected the same role contract into leader/worker default prompts and removed
    `.cache/context/todo.md` from startup guidance.
- `web/src/pages/team/member_helpers.ts`
  - aligned the Team UI prompt previews with the same `.agenthubmemory` contract.
- `src/state.rs`
  - added startup logic that ensures `$HOME/.gitignore_global` and Git's default user ignore path
    both contain `.agenthubmemory`, so the protection still works even when `core.excludesfile`
    is not pointed at `.gitignore_global`.
- `docs/features/agents-teams.md`
  - updated the stable Team feature spec with the `.agenthubmemory/` ownership rule.
- `docs/todo.md`
  - updated the verification backlog to reflect the new startup behavior.

## Validation

- `cargo test -p agenthub-team-prompts`
- `cargo test ensure_global_gitignore_contains_agenthubmemory_entry`
- `cargo test ensure_global_gitignore_keeps_agenthubmemory_entry_idempotent`
- `cargo test ensure_global_gitignore_prefers_xdg_config_home_when_present`

## Follow-up

- later wire the same contract into non-Team Agent bootstrap flows where project-local memory is
  also needed.
