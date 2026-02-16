# Proto Codegen Cargo/Bazel Convergence

## Summary

Converge Cargo and Bazel protobuf usage on a single contract:

- `proto/internal/v1/team.proto` is the source of truth.
- `src/internal/proto/agenthub.internal.v1.rs` is the tracked generated artifact.
- CI verifies the tracked artifact is byte-identical to fresh codegen output.

## Background

After Bazel migration, runtime proto inclusion moved to the tracked generated file
path, while some docs and workflow checks still reflected the old
"generated file is not checked in" model. This caused ambiguity in developer
workflow and left Rust CI without an active proto drift gate.

## Scope

- `scripts/check_team_proto_codegen.sh`
- `Makefile`
- `.github/workflows/rust.yml`
- `README.md`
- `docs/todo.md`

## Key Decisions

1. Keep `team.proto` as schema source and keep tracked generated Rust output at
   `src/internal/proto/agenthub.internal.v1.rs`.
2. Extend `scripts/check_team_proto_codegen.sh` with explicit modes:
   - `--write`: refresh tracked generated file from latest codegen output
   - `--check`: verify tracked generated file matches latest codegen output
3. Add `make proto-gen` and keep `make proto-check` as the standard workflow.
4. Re-enable Rust CI proto verification with `Verify internal protobuf codegen`
   step (`make proto-check`) before regular Cargo build/test steps.
5. Update README to document tracked generated file policy and the new
   `proto-gen` + `proto-check` commands.

## Validation

```bash
make proto-check
```

Expected:

- command succeeds when tracked generated file matches current codegen output;
- command fails when `team.proto` changes are not reflected in
  `src/internal/proto/agenthub.internal.v1.rs`;
- Rust CI fails early on proto drift.

## Follow-ups

- Consider extracting shared proto generation logic into a dedicated helper to
  avoid relying on Cargo build output path discovery.
