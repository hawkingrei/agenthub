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
- Added/kept focused tests for:
  - managed skill path layout and install behavior
  - prompt prefix ordering with dynamic runtime context
  - Codex conversion from ACP `<skill>` wrappers to native `UserInput::Skill`

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
