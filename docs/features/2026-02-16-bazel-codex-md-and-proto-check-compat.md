# Bazel Codex Markdown Input and Proto Check Compatibility

## Summary

Fix CI failures after syncing `main` by:

1. Declaring `agenthub-codex-acp` markdown prompt files as Bazel compile inputs.
2. Making protobuf codegen check script compatible with branches that still keep
   tracked generated proto Rust output.

## Background

After merging `main` into the Bazel integration branch, CI failed in two places:

- Bazel build failed because `include_str!("./prompt_for_init_command.md")`
  could not find the markdown file under Bazel sandbox inputs.
- Rust workflow failed in `scripts/check_team_proto_codegen.sh` because it
  rejected any tracked `agenthub.internal.v1.rs` file, while this branch still
  relies on that tracked file path.

## Scope

- `agenthub-codex-acp/BUILD.bazel`
- `scripts/check_team_proto_codegen.sh`
- `docs/todo.md`

## Key Decisions

1. Add `compile_data = glob(["src/**/*.md"])` to
   `//agenthub-codex-acp:agenthub_codex_acp` so markdown prompt files are
   available to `include_str!` during Bazel compilation.
2. Update proto check script behavior:
   - still run `cargo check` and locate generated proto output;
   - still validate expected symbols in generated file;
   - if tracked `src/internal/proto/agenthub.internal.v1.rs` exists, require it
     to be byte-identical with latest generated output instead of failing
     unconditionally.

## Validation

```bash
gh pr checks 22
```

Expected:

- `Rust (Bazel Targets)` no longer fails in the proto check step.
- `Bazel Build and Test` no longer fails on missing
  `prompt_for_init_command.md`.

## Follow-ups

- Complete migration to build-time proto include (`OUT_DIR`) and then re-enable
  strict "no tracked generated proto file" policy.
