# Summary

AgentHub's Codex ACP adapter now targets Codex `rust-v0.135.0`.

# Background

The repository had drifted onto a mixed Codex graph with most ACP-facing crates
still pinned to `rust-v0.133.0`, `codex-apply-patch` already at `0.134.0`, and
Bazel still consuming the older `codex_src` commit. This slice normalizes the
workspace onto the upstream `0.135.0` release and updates the ACP bridge for
the protocol changes that landed between `0.133` and `0.135`.

# Scope

- Updated `agenthub-codex-acp` Codex git dependencies to `rust-v0.135.0`.
- Updated `Cargo.lock` to the upstream `rust-v0.135.0` release commit
  `4daceea869704f9f35e0a3949fc34711ef978a4e`.
- Updated `MODULE.bazel` `codex_src` to the same upstream release commit.
- Refreshed the Bazel `rusty_v8` prebuilt pin from `146.4.0` to `147.4.0`,
  including new archive hashes and target aliases under `third_party/v8`.
- Adapted ACP bridge code for Codex `0.135` protocol/config changes around
  turn start/steer context, trace propagation, and MCP server environment ids.

# Key Decisions

- Keep ACP-side follow-up prompts explicit by populating `additional_context`
  with empty defaults for every local `Op::UserInput` constructor, and `None`
  for `TurnStartParams` / `TurnSteerParams` until AgentHub exposes a real
  turn-scoped additional-context surface.
- Preserve `TurnStartedEvent` compatibility by setting the new optional
  `trace_id` field to `None` in local synthetic events and ignoring it in ACP
  event pattern matches.
- Derive the user profile override from
  `config.config_layer_stack.get_active_user_layer()` instead of the removed
  `Config.active_profile` field so app-server resume requests still reflect the
  active profile-v2 overlay.
- Update MCP server projection to use Codex `0.135`'s required
  `environment_id` field with `DEFAULT_MCP_SERVER_ENVIRONMENT_ID`, replacing
  the removed `experimental_environment` field.

# Validation

```bash
cargo fmt --all
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo test -p agenthub-codex-acp
```

# Follow-Ups

- Run the normal repository CI after this lands to cover non-ACP packages and
  Bazel integration on the refreshed Codex `0.135.0` lockfile.
- Decide separately whether AgentHub should surface real
  `turn/start.additionalContext` / `turn/steer.additionalContext` data once the
  Team/mailbox runtime grows a durable source for model-visible contextual
  fragments.
