# Team Memory Index Schema Metadata

## Summary

- added shared Team runtime memory-index helpers under `agenthub-team-domain` so runtime and ACP
  pointer paths stop drifting independently
- started writing explicit schema metadata into the machine-read Team runtime files
  `.cache/context/state.md` and `.cache/context/run/<run_id>/continuity.md`
- added compatibility-focused tests for both legacy artifact-pointer shape handling and the new
  schema metadata lines

## Why

The rolling-upgrade spec now says machine-read Team memory/index files should behave like versioned
file protocols.

Without shared helpers and explicit schema markers, `state.md` and `continuity.md` would still be
easy to evolve inconsistently across runtime writers and ACP readers.

## Changes

- `crates/agenthub-team-domain/src/lib.rs`
  - added:
    - `TEAM_RUNTIME_STATE_SCHEMA_FAMILY`
    - `TEAM_RUNTIME_STATE_SCHEMA_VERSION`
    - `TEAM_CONTINUITY_NOTE_SCHEMA_FAMILY`
    - `TEAM_CONTINUITY_NOTE_SCHEMA_VERSION`
    - `TEAM_RUNTIME_STATE_RELATIVE_PATH`
    - `continuity_note_relative_path(...)`
    - `extract_context_artifact_path(...)`
- `src/team/manager.rs`
  - `state.md` now writes:
    - `schema_family: team_runtime_state`
    - `schema_version: 1`
  - `continuity.md` now writes:
    - `schema_family: team_continuity_note`
    - `schema_version: 1`
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
  - switched continuity note path and artifact pointer extraction to the shared Team-domain helpers
- tests
  - Team manager regression now asserts schema metadata in both runtime files
  - Team-domain unit tests cover object-shaped and legacy string-shaped artifact pointers
  - Team-domain parsers now cover:
    - current `state.md` schema metadata parsing
    - legacy `state.md` parsing without schema fields
    - `continuity.md` header parsing without reading into summary/history body

## Follow-Up

- future machine-read Team context files should use the same schema metadata pattern instead of
  ad-hoc field-only contracts
- the next prompt-tail slimming change can build on these helpers when adding more
  compatibility-facing pointers to `state.md`
