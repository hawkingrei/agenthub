# OpenAPI Package Module Migration

## Background

OpenAPI API code still relied on a single file entry (`src/api/openapi.rs`) even after extracting spec assembly. Router handlers, docs-page HTML, and tests lived in the same file, which kept module boundaries less explicit than other refactored areas (for example ACP package layout).

## Scope

- Migrate OpenAPI route module from single-file layout to directory package layout:
  - remove `src/api/openapi.rs`
  - add `src/api/openapi/mod.rs`
  - add `src/api/openapi/docs_page.rs`
  - keep `src/api/openapi/spec.rs`
  - add `src/api/openapi/tests.rs`
- Preserve endpoint behavior:
  - `GET /api/openapi.json`
  - `GET /api/openapi/docs`

## Key Decisions

- Keep `mod.rs` focused on route wiring and handlers only.
- Move docs HTML payload to `docs_page.rs` to isolate static content from handler logic.
- Move OpenAPI module tests to dedicated `tests.rs` for cleaner module-level evolution.
- Keep existing `spec.rs` payload construction unchanged in this migration to avoid semantic changes.

## Validation

Suggested checks:

```bash
cargo fmt --all --check
cargo test openapi_json_ --lib
cargo test health_returns_ok --lib
```

Expected:

- formatter passes
- OpenAPI auth/path tests remain green
- top-level API health route test remains green

## Follow-up

- Continue splitting `src/api/openapi/spec.rs` into `components` / `paths` helpers (or typed generator) to complete the pending complexity-reduction TODO.
