# Codex ACP Upstream Sync: 3b80a66 API Compatibility Fixes

## Summary

Align `agenthub-codex-acp` with codex upstream API changes introduced around:

- `https://github.com/zed-industries/codex-acp/commit/3b80a66ff118f19cdfeab21511caa61924982d29`
- codex git revision used in workspace lockfile: `c34b30a`

The adapter now compiles again after dependency update.

## Scope

- `agenthub-codex-acp/src/lib.rs`
- `agenthub-codex-acp/src/main.rs`
- `agenthub-codex-acp/src/codex_agent.rs`
- `agenthub-codex-acp/src/thread.rs`
- `docs/todo.md`

## Background

After upstream update, several API shapes moved or changed:

1. `codex_common` exports used by ACP adapter moved to utility crates.
2. protocol events gained new required fields (`turn_id`, `status`).
3. `Config` permission fields moved under `Config.permissions`.
4. `SandboxPolicy::ReadOnly` became a struct variant.
5. `McpServerConfig` now requires `required`.
6. `RolloutRecorder::list_threads` now accepts `&Config`.
7. `EventMsg` gained collab resume variants, requiring exhaustive match updates.

## Key Decisions

1. Follow upstream crate boundaries directly:
   - `CliConfigOverrides` from `codex_utils_cli`
   - approval presets from `codex_utils_approval_presets`
2. Prefer forward-compatible event destructuring (`..` or explicit ignored fields)
   where fields are not consumed.
3. Keep runtime behavior unchanged while adapting to new struct layouts:
   - mode matching compares against `self.config.permissions.*`
   - mode updates write back through `self.config.permissions.*`
4. Keep MCP server defaults conservative by setting `required: false`.

## Validation

```bash
cargo check -p agenthub-codex-acp
```

Expected outcome:

- `agenthub-codex-acp` compiles cleanly against current codex dependencies.
- no unresolved imports from `codex_common`.
- no missing-field errors for updated protocol/config structs.
