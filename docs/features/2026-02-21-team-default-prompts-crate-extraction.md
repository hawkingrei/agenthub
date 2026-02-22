# Team Default Prompts Crate Extraction

## Background

`src/api/teams.rs` embedded large default prompt constants for leader/worker roles. The prompt payloads are long and change-prone, which made API router code harder to scan and maintain.

## Scope

- `crates/agenthub-team-prompts/Cargo.toml`
- `crates/agenthub-team-prompts/src/lib.rs`
- `Cargo.toml`
- `src/api/teams.rs`
- `docs/todo.md`

## Key Decisions

1. Introduce a dedicated workspace crate `agenthub-team-prompts` for default Team prompt templates.
2. Keep exported API minimal:
   - `DEFAULT_TEAM_LEADER_PROMPT`
   - `DEFAULT_TEAM_WORKER_PROMPT`
   - `default_team_prompt_for_role(role: &str) -> &'static str`
3. Preserve existing runtime behavior in Team spec default injection:
   - when `spec.members[].prompt` is missing/null, resolve by member role;
   - unknown role fallback remains worker prompt, matching previous `if role == "leader" else worker` behavior.
4. Remove inline prompt constants from `src/api/teams.rs`; keep Team skills/default-step logic local to API module.

## Validation

- `cargo fmt`
- `cargo test -p agenthub-team-prompts`
- `cargo test teams_api_rejects_spec_with_too_many_steps -- --nocapture`

