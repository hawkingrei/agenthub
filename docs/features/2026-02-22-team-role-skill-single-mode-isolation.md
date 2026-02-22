# Team Role Skill Isolation For Single-Mode Sessions

## Background

ACP sessions load global skills from `~/.agenthub/skills.json`.
When that file contains Team role skills (`team-leader-orchestrator`, `team-worker-executor`,
`team-deliberation-rules`), single-mode/manual agent sessions also receive these Team-only
instructions, which is not desired.

## Scope

- `crates/agenthub-acp/src/lib.rs`
- `crates/agenthub-acp/src/team_role_skills.rs`

## Key Decisions

1. Treat Team role skills as reserved runtime skills instead of ordinary global skills.
2. Strip reserved Team role skill names from globally loaded `skills.json` entries for all ACP
   sessions.
3. Inject Team role skills only when actor runtime context explicitly carries a supported Team role
   (`leader` or `worker`).
4. Keep actor runtime skill injection unchanged for actor-context sessions.

## Validation

Executed:

```bash
cargo fmt --all
cargo test -p agenthub-acp team_role_skills
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Observed:

- Team role skill helper tests passed.
- Workspace clippy check passed with `-D warnings`.
- Single-mode isolation logic compiles and is covered by role-gating unit tests.
