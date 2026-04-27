# Summary

Move AgentHub-managed Codex skills onto stable files under
`~/.agents/skills/agenthub-runtime/.../SKILL.md`, keep dynamic actor runtime
state in a separate prompt text block, and let `agenthub-codex-acp` translate
the resulting absolute-path ACP `<skill>` wrappers into native Codex
`UserInput::Skill` items.

# Why

AgentHub still needs ACP-compatible skill injection because ACP has no standard
skill content block type. Codex, however, already has a native file-backed
skill path. The clean bridge is:

1. Materialize stable `SKILL.md` files under the user skill root Codex already
   scans.
2. Inject those files through AgentHub's existing ACP `<skill>` text wrapper.
3. Teach `agenthub-codex-acp` to translate absolute-path wrappers into native
   `UserInput::Skill`.

Dynamic fields such as `team_id`, `current_run_id`, `actor_id`, and continuity
summary must not be written into those stable skill files, otherwise the files
would become run-specific and difficult to manage. Those fields now belong in a
separate runtime context text block injected before each prompt.

This design intentionally avoids editing `~/.codex/config.toml` or introducing a
separate AgentHub-specific Codex skill toggle.

# What Changed

- Added `crates/agenthub-managed-skills` to generate and install the
  AgentHub-managed Team/runtime skill set under
  `~/.agents/skills/agenthub-runtime/.../SKILL.md`.
- Updated `agenthub-acp` Team and actor runtime skill builders to prefer those
  stable absolute paths when the managed skill files exist.
- Added best-effort managed skill installation during ACP session bootstrap so
  Codex-backed sessions can use native skill paths without a separate bootstrap
  command.
- Split the old actor runtime skill payload into:
  - a static `agenthub-actor-runtime` managed skill file
  - a dynamic runtime context text block injected before each prompt
- Kept the existing text fallback behavior when managed files cannot be
  materialized, preserving compatibility for non-Codex ACP providers.
- Restricted Codex native skill conversion to canonical `SKILL.md` files under
  the AgentHub-managed `~/.agents/skills/agenthub-runtime/...` root instead of
  accepting arbitrary absolute paths from prompt text.
- Made managed skill installation idempotent and temp-file-backed so repeated
  ACP session bootstrap does not rewrite unchanged skill files in place.
- Followed up on review hardening:
  - skip managed skill installation quietly when no home directory can be
    resolved for the current process
  - canonicalize the managed skill trust root once per prompt build instead of
    once per skill block
  - retry temp-file allocation on `AlreadyExists` and add an in-process counter
    to reduce collision risk under concurrent bootstrap
- Added/kept focused tests for:
  - managed skill path layout and install behavior
  - prompt prefix ordering with dynamic runtime context
  - Codex conversion from ACP `<skill>` wrappers to native `UserInput::Skill`
    only for AgentHub-managed skill files

# Scope Boundary

This work does not add an ACP-facing `skills/list` bridge and does not modify
Codex user config. It only improves the injection path used when AgentHub sends
skills into Codex-backed ACP sessions.

# Validation

Suggested validation:

- `cargo test -p agenthub-managed-skills`
- `cargo test -p agenthub-acp`
- `cargo test -p agenthub-codex-acp build_prompt_items`
- Manual ACP session check confirming that AgentHub-managed skills are present
  under `~/.agents/skills/agenthub-runtime/...` and that Codex receives native
  skill inputs for those absolute paths.

Expected result:

- AgentHub-managed Team/runtime skills are file-backed and stable under
  `~/.agents/skills/agenthub-runtime/...`.
- `agenthub-codex-acp` converts those absolute-path wrappers into native Codex
  skill inputs.
- Dynamic actor runtime data appears as a separate text prefix block rather than
  being baked into the managed skill files.

# 2026-04-27 Verification Follow-up

This path now has a tighter automated verification chain:

- `cargo test -p agenthub-managed-skills -- --nocapture`
  - confirms the managed skill documents are materialized under the canonical
    `~/.agents/skills/agenthub-runtime/.../SKILL.md` namespace
- `cargo test -p agenthub-acp prompt_prefix_blocks_keep_managed_skill_file_static_and_runtime_context_dynamic -- --nocapture`
  - confirms ACP prompt prefix assembly keeps the managed actor runtime skill as
    a stable absolute-path `<skill>` wrapper and appends the dynamic runtime
    identity block separately
- `agenthub-codex-acp/src/thread.rs` continues to carry the focused
  `build_prompt_items_*` tests that translate trusted managed skill wrappers
  into native Codex `UserInput::Skill` items while preserving ordinary text
  blocks as `UserInput::Text`

Local verification result on this machine:

- the managed-skill materialization and ACP prompt-prefix tests passed locally
- a fresh `cargo test -p agenthub-codex-acp test_build_prompt_items_ -- --nocapture`
  attempt did not reach the test assertions because the local target directory
  ran out of space during dependency compilation (`No space left on device`)

Because the adapter-side focused test did not complete on this machine, keep the
umbrella TODO open until the `agenthub-codex-acp` translation test run (or
equivalent CI evidence) is recorded alongside these materialization and
prompt-prefix checks.
