# Summary

AgentHub's Codex ACP adapter now targets Codex `rust-v0.133.0`.

# Background

The repository was previously pinned to Codex `rust-v0.132.0`. This upgrade
refreshes both the Cargo git dependencies and the Bazel `codex_src` pin while
adapting the ACP bridge to Codex 0.133 protocol changes.

# Scope

- Updated `agenthub-codex-acp` Codex git dependencies from `rust-v0.132.0` to
  `rust-v0.133.0`.
- Updated `Cargo.lock` to the upstream `rust-v0.133.0` release commit
  `9474e5cfc4494b0ba319352aa86ce436c59e65c8`.
- Updated `MODULE.bazel` `codex_src` to the same upstream release commit.
- Adapted ACP bridge code for Codex 0.133 thread settings and MCP tool event
  shape changes.
- Refreshed root ICU pins so the workspace lockfile can resolve Codex 0.133's
  newer `v8`/ICU dependency graph.

# Key Decisions

- Translate legacy ACP config updates onto Codex 0.133's new
  `Op::ThreadSettings` contract instead of keeping a local compatibility shim
  around `Op::OverrideTurnContext`.
- Keep prompt submissions explicit by populating
  `thread_settings: ThreadSettingsOverrides::default()` for all local
  `Op::UserInput` constructors.
- Preserve MCP event compatibility by setting the newly added optional
  `plugin_id` field to `None` until AgentHub exposes plugin-aware MCP reporting.
- Treat app-server `ThreadSettingsUpdated` notifications as a no-op in this
  slice; the ACP bridge now accepts the new notification without trying to
  project the full app-server thread settings snapshot into user-facing session
  updates.
- Bump the root workspace's ICU overrides to `icu_calendar = 2.2.1` and
  `icu_decimal = 2.2.0` because the previous `2.1.1` pins prevented Cargo from
  resolving Codex 0.133's transitive `v8` dependency tree.

# Validation

```bash
cargo check -p agenthub-codex-acp
cargo check -p agenthub-codex-acp --tests
cargo test -p agenthub-codex-acp
```

# Follow-Ups

- Run the normal repository CI after this lands to cover non-ACP packages and
  Bazel integration on the refreshed Codex 0.133 lockfile.
- Decide separately whether AgentHub should surface app-server
  `ThreadSettingsUpdated` notifications as a user-visible ACP session event or
  continue treating them as internal runtime metadata.
