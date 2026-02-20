# ACP Package And Bootstrap Module Migration

## Background

ACP-related code paths were split between:

- `src/acp.rs` (ACP event sink + ACP crate re-export)
- `src/actor_runtime.rs` (actor runtime context normalization helpers)
- `src/lib.rs` (top-level service bootstrap logic)

This made ACP boundaries less explicit and kept startup orchestration mixed with module registration in `lib.rs`.

## Scope

- Introduce ACP package-style module layout under `src/acp/`:
  - `src/acp/mod.rs`
  - `src/acp/event_sink.rs`
  - `src/acp/runtime.rs`
- Move `actor_runtime` helpers into ACP package and update call sites to `crate::acp::*`.
- Deduplicate actor default channel constant usage in `actor_mcp` by reusing ACP runtime constant.
- Move service bootstrap (`run`) from `src/lib.rs` into `src/app.rs`, and keep `lib.rs` focused on module wiring + export.

## Key Decisions

- Keep ACP external contract unchanged by preserving `pub use agenthub_acp::*` at `src/acp/mod.rs`.
- Keep runtime helpers crate-internal (`pub(crate)`) while exposing them through ACP package boundary for internal consumers.
- Keep binary entry (`src/main.rs`) minimal and stable (`agenthub::run().await`), while relocating startup orchestration to `src/app.rs` for maintainability.

## Validation

Suggested verification commands:

```bash
cargo fmt --all
cargo test openapi_json_ --lib
cargo test default_actor_cli_path_returns_non_empty_value --lib
cargo test parse_start_actor_runtime_context --lib
cargo test parse_actor_mcp_context --lib
```

Expected outcome:

- all commands pass
- ACP runtime/context tests remain green
- actor-mcp context parsing tests remain green
- OpenAPI auth/path tests remain green

## Follow-up

- If ACP responsibilities continue to grow, split `src/acp/runtime.rs` into context normalization and actor-cli path policy submodules.
- Consider a future `src/app/bootstrap.rs` + `src/app/router.rs` split when startup/router wiring expands further.
