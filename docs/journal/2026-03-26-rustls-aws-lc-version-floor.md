# Rustls And AWS-LC Version Floor Refresh

## Summary

- raised the top-level `rustls` dependency floor from `0.23` to `0.23.37`
- confirmed the workspace lockfile already resolves `rustls` to `0.23.37`
- confirmed `aws-lc-rs` is still transitively selected through `rustls` and now resolves to the
  patched `1.16.2` line

## Scope

- `.bazelignore`
- `Cargo.toml`
- `docs/todo.md`
- `docs/journal/2026-03-26-rustls-aws-lc-version-floor.md`

## Notes

- `aws-lc-rs` is not a direct workspace dependency today; it is pulled by `rustls` and related TLS stack crates
- keeping `aws-lc-rs` transitive avoids adding an otherwise unused direct dependency just to influence the lockfile
- a minimal `.bazelignore` now keeps local build/cache directories such as `.cargo` and `target` out of `bazel build //...` package discovery
- `Cargo.lock` already resolved the latest stable `rustls` / `aws-lc-rs` pair, so no lockfile edit was required for this change
- `cargo tree -i rustls -e normal` and `cargo tree -i aws-lc-rs -e normal` are the intended inspection commands for checking the final resolved TLS stack locally

## Validation

- verify the dependency graph still resolves one `rustls v0.23.37`, `aws-lc-rs v1.16.2`, and
  `aws-lc-sys v0.39.0`
- run `cargo check`
- run `bazel build //...`
