# Dependabot Alert Remediation

## Summary

Remediate the active Dependabot security alerts in the main workspace lockfiles:

- Rust `aws-lc-sys` in `Cargo.lock`
- npm `serialize-javascript` in `userdocs/package-lock.json`

## Why

The repository had active GitHub Dependabot alerts for:

- `GHSA-394x-vwmw-crm3` on `aws-lc-sys`, requiring `aws-lc-sys >= 0.39.0`
- `GHSA-qj8w-gfj5-8c6v` on `serialize-javascript`, requiring `serialize-javascript >= 7.0.5`

Both alerts were lockfile-driven and should be remediated at the dependency resolution layer.

Refreshing `aws-lc-rs` / `aws-lc-sys` to the patched line exposed a Bazel-only incompatibility in
`aws-lc-sys 0.39.0`: the build script now emits a symlinked `rust_wrapper.h`, while the
crate-universe sandbox treated that entry as a directory copy target and failed before Rust
compilation started. The final remediation keeps the patched Rust lockfile and adds a narrow Bazel
patch for `aws-lc-sys` so Cargo and Bazel can build the same dependency set.

## Changes

- refresh the Rust lockfile so `aws-lc-rs` / `aws-lc-sys` move to the patched release line
- patch the Bazel `crate_universe` copy step for `aws-lc-sys` so symlinked header files are copied
  as files instead of being treated as directories
- add a `userdocs/package.json` override so the Docusaurus webpack chain resolves
  `serialize-javascript` to `7.0.5` or newer
- regenerate `userdocs/package-lock.json`

## Validation

Local validation for this change should cover:

- `cargo check --workspace --locked`
- `cargo test --locked`
- `cargo tree --workspace -i aws-lc-sys`
- `bazel build //...`
- `npm --prefix userdocs ci`
- `npm --prefix userdocs run build`
- confirm the GitHub Dependabot alerts are auto-dismissed after the updated lockfiles land on default branch

## Follow-up

- Record the first green CI run IDs and the Dependabot alert dismissal evidence before removing the
  related verification item from `docs/todo.md`.
