# 2026-03-25 Managed Skills No Builtin Fallback

## Summary

- removed `builtin://...` fallback for Team role managed skills and the actor runtime skill
- require managed Team/actor skills to exist as materialized `SKILL.md` files under `~/.agents/skills/agenthub-runtime/...`
- changed ACP actor session startup to fail fast when managed skill installation or loading fails

## Why

Team actor sessions were still able to degrade into inline `builtin://...` skill paths when
managed skill materialization failed. That kept the session alive, but it also:

- hid managed skill installation problems
- prevented `agenthub-codex-acp` from upgrading trusted skill blocks into native skill inputs
- increased prompt/context overhead because the fallback path stayed text-like

For Team runtime correctness and prompt efficiency, managed Team/actor skills need to resolve to
real `SKILL.md` paths consistently.

## Implementation

- `crates/agenthub-acp/src/actor_runtime_skill.rs`
  - introduced a strict `build_required_managed_skill(...)` helper
  - `build_actor_runtime_skill()` now returns an error if the managed skill is not materialized
  - materialization validation now requires a regular file, not only path existence
  - startup error text is generic to ACP session startup, so Team role skill failures report accurately
- `crates/agenthub-acp/src/team_role_skills.rs`
  - Team role skill builders now return `Result<_>` and require materialized managed skill files
  - tests now install managed skills into a temp home instead of relying on ambient `~/.agents`
- `crates/agenthub-acp/src/lib.rs`
  - `spawn_acp_session(...)` now fails session startup if:
    - managed skill installation fails
    - Team role skill loading fails
    - actor runtime skill loading fails
  - reserved Team/actor managed skills now evict conflicting preloaded skills by canonical name/path before dedupe, so the materialized runtime skills always win over workspace/config aliases

## Validation

- `cargo test -p agenthub-acp build_team_role_skills -- --nocapture`
- `cargo test -p agenthub-acp actor_runtime_skill -- --nocapture`
- `cargo test -p agenthub-acp prompt_prefix_blocks -- --nocapture`
- `git diff --check`
