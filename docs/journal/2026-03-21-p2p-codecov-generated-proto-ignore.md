# P2P Codecov Generated Proto Ignore

## Summary

- Added a root `codecov.yml` to exclude `src/internal/proto/agenthub.internal.v1.rs` from patch coverage.
- Kept the change minimal: do not relax Codecov thresholds and do not exclude hand-written runtime logic.

## Rationale

- `src/internal/proto/agenthub.internal.v1.rs` is generated from `proto/internal/v1/team.proto`.
- Patch coverage for PR `#128` was blocked mainly by this generated file even after the hand-written P2P/runtime paths were covered.
- Excluding generated transport stubs keeps Codecov focused on reviewer-owned logic instead of prost/tonic output.

## Validation

- Parse `codecov.yml` as plain YAML.
- Re-run PR `#128` checks and confirm `codecov/patch` clears with the generated proto ignored.
