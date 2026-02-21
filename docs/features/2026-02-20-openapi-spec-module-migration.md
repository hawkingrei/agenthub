# OpenAPI Spec Module Migration

## Background

OpenAPI route module previously mixed three concerns in one large file:

- OpenAPI route handlers (`/api/openapi.json`, `/api/openapi/docs`)
- OpenAPI JSON document assembly
- Embedded docs-page HTML

The spec assembly function was large enough to make router/handler reviews noisy and to increase merge conflict probability for unrelated handler changes.

## Scope

- Move OpenAPI spec assembly into a dedicated submodule:
  - `src/api/openapi/spec.rs`
- Keep OpenAPI route module focused on:
  - route wiring
  - auth guard handling for `/openapi.json`
  - docs-page HTML response
- Preserve existing OpenAPI payload shape and endpoint behavior.

## Key Decisions

- Keep spec evolution colocated with the OpenAPI route module via `mod spec;`.
- Keep the public surface minimal (`pub(super) fn openapi_spec()`) and avoid exposing spec internals outside the `api::openapi` module.
- Do not perform schema/path semantic edits in this change; this is a structural migration only.

## Validation

Suggested verification commands:

```bash
cargo fmt --all --check
cargo test openapi_json_ --lib
```

Expected checks:

- formatter passes with no diff
- `api::openapi::tests::openapi_json_requires_authorization`
- `api::openapi::tests::openapi_json_contains_team_runs_list_path`

## Follow-up

- Continue the refactor by splitting spec assembly into `components` and `paths` helpers inside `src/api/openapi/spec.rs` if future OpenAPI expansion increases review friction.
