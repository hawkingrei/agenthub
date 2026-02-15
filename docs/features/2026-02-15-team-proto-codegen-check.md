# Team Proto Codegen Check

## Summary

Add a reproducible protobuf codegen verification workflow for
`proto/internal/v1/team.proto`, and document how developers trigger
regeneration/checks locally.

## Background

Generated protobuf Rust stubs are derived from `team.proto`. Without an explicit
check and workflow guidance, schema updates can drift from generated output
expectations and cause review/CI confusion.

## Scope

- `.github/workflows/rust.yml`
- `scripts/check_team_proto_codegen.sh`
- `Makefile`
- `README.md`
- `proto/internal/v1/team.proto`
- `docs/todo.md`

## Key Decisions

1. Keep protobuf Rust generation at build-time via `tonic-build` (`build.rs`)
   instead of checking generated Rust files into git.
2. Add a dedicated script (`scripts/check_team_proto_codegen.sh`) that:
   - runs `cargo check --locked` to trigger codegen;
   - validates generated output file discovery;
   - validates expected generated symbols exist;
   - fails if generated protobuf Rust files are tracked in git.
3. Wire the check into Rust CI so schema/codegen regressions are caught before
   merge.
4. Add explicit proto-file header comments and README instructions for local
   developer workflow.

## Validation

```bash
make proto-check
```

Expected outcomes:

- command succeeds when `team.proto` and codegen pipeline are healthy;
- command fails when generated symbols are missing;
- command fails when checked-in generated protobuf Rust files are detected.

## Follow-ups

- Evaluate whether a Bazel-native protobuf generation path should replace the
  current Cargo build-time pipeline for large multi-service deployments.
