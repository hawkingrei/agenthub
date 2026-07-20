# Release Vendored OpenSSL And Partial Assets

## Scope

Align the GitHub release workflow with two concrete requirements:

1. successful targets should still publish binary assets when another target in the release matrix fails;
2. Linux release cross-builds should stop binding to the stale OpenSSL 1.0.2 sysroot that ships in the default `cross` images.

## Changes

- changed `.github/workflows/release.yml` so the `release` job always runs after the build matrix, downloads whatever `release-*` artifacts exist, and only fails when no archives were produced at all;
- generated the release body from the actual `dist/` contents so partial releases no longer advertise assets that were never built;
- added `.github/workflows/release-prebuild.yml` so merges to `main` exercise the same release-target matrix and packaging path before a tag is cut;
- removed `libssl-dev` installs from Linux `Cross.toml` pre-build hooks and explicitly installed `zlib1g-dev` so the cross sysroot no longer injects OpenSSL 1.0.2 into release builds while still keeping compression headers available;
- moved vendored OpenSSL behind release-only Cargo features (`release-vendored-openssl`) in both `agenthub` and the ACP adapter stack so Linux release builds still converge on vendored OpenSSL without dragging `openssl-src` into the default Bazel / crate_universe dependency graph.
- kept vendored OpenSSL scoped to Linux cross release/prebuild legs only, excluded source-only archives from the release artifact guard, and removed `continue-on-error` from artifact download so infrastructure failures still stop the release job;
- added a Linux `memfd_create` compatibility shim in `agenthub-codex-acp` so V8-backed release/prebuild links no longer depend on the older GNU cross sysroot exporting that libc wrapper.

## Validation

### 2026-06-11 Release Prebuild Verification

- Observed main `Release Prebuild` run `27359976172` after merge commit `6e64fe95e1517895a23686de0f5d1be34136ad35`.
- Confirmed the workflow triggers on `push` to `main` and starts the same release-target matrix:
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and `aarch64-apple-darwin`.
- Confirmed the run completed successfully:
  - `Prebuild x86_64-unknown-linux-gnu`: success, 1h12m13s.
  - `Prebuild aarch64-unknown-linux-gnu`: success, 1h20m49s.
  - `Prebuild aarch64-apple-darwin`: success, 38m50s.
- Confirmed both Linux `agenthub` and `agenthub-acp` cross builds passed with
  `release-vendored-openssl`; the stale cross-sysroot OpenSSL failure did not recur for the primary
  release binary or canonical ACP adapter.
- Downloaded the produced artifacts and confirmed the current workflow still emits:
  - `agenthub-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
  - `agenthub-acp-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
  - `agenthub-codex-acp-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
- Found that the prebuild still spent release time building and packaging the legacy
  `agenthub-codex-acp` binary after `agenthub-acp` became the canonical ACP release entrypoint.
  The follow-up change removes `agenthub-codex-acp` from release and prebuild package outputs while
  leaving compatibility detection and internal adapter reuse intact.
- Found a non-fatal `dtolnay/rust-toolchain` annotation for the unsupported `profile` input. The
  follow-up change removes that input from both release workflows.

Planned validation for the next release-tag or preview-tag run:

- confirm `Build x86_64-unknown-linux-gnu` and `Build aarch64-unknown-linux-gnu` no longer panic in `openssl-sys`;
- confirm Linux `agenthub-acp` release/prebuild links no longer fail on V8 / libc compatibility
  symbols;
- confirm a failing matrix leg no longer suppresses successful release assets from the GitHub Release page;
- confirm `Release Prebuild` catches the same cross-build regressions on `push` to `main` before a
  tag is cut;
- record the release workflow run IDs and the resulting release URLs in this note before closing the follow-up TODO item.

### 2026-07-20 Release Prebuild Scope Audit

- Observed main `Release Prebuild` run `29694176620` after merge commit
  `909b1fb3f8eb8a90ebf02e3724afdf9e95148b9c`.
- Confirmed the run completed successfully and uploaded exactly three matrix artifacts:
  - `release-prebuild-aarch64-apple-darwin`
  - `release-prebuild-x86_64-unknown-linux-gnu`
  - `release-prebuild-aarch64-unknown-linux-gnu`
- Confirmed package logs still called `package_binary "agenthub"` and `package_binary
  "agenthub-acp"` only, so the prebuild artifact contents stayed on the canonical release
  entrypoints.
- Confirmed the same run still compiled the internal package as `agenthub-codex-acp v0.10.0`
  through the `agenthub-acp-adapter` release build. That means the artifact trimming had landed, but
  the release build scope still exposed the old package identity.
- Renamed the internal package to `agenthub-codex-acp-runtime` while preserving the compatibility
  binary name `agenthub-codex-acp` and keeping `agenthub-acp` as the canonical packaged ACP
  entrypoint.
- Local validation:

```bash
cargo check -p agenthub-acp-adapter
```

Remaining validation before closing the TODO:

- confirm the next `Release Prebuild` push-to-main run compiles `agenthub-codex-acp-runtime`
  instead of `agenthub-codex-acp`;
- confirm the same run still uploads only `release-prebuild-{target}` artifacts whose package logs
  include `agenthub` and `agenthub-acp` archives plus Linux `.deb` packages;
- confirm the next semver release or preview release still publishes successful binary assets even
  if one matrix target fails.

### 2026-07-20 Release Prebuild Runtime Package Verification

- Observed main `Release Prebuild` run `29748545683` after merge commit
  `676230bf6d664eb56559d5ed96fa3fa4ca44a136`.
- Confirmed the run completed successfully:
  - `Prebuild x86_64-unknown-linux-gnu`: success, 1h10m59s.
  - `Prebuild aarch64-unknown-linux-gnu`: success, 1h16m38s.
  - `Prebuild aarch64-apple-darwin`: success, 43m53s.
- Confirmed the release/prebuild matrix still exercised the canonical release build commands:
  - `cross build --locked --release --target x86_64-unknown-linux-gnu --bin agenthub --features release-vendored-openssl,release-lance-fp16,rocksdb`
  - `cross build --locked --release --target x86_64-unknown-linux-gnu -p agenthub-acp-adapter --features release-vendored-openssl`
  - `cross build --locked --release --target aarch64-unknown-linux-gnu --bin agenthub --features release-vendored-openssl,rocksdb`
  - `cross build --locked --release --target aarch64-unknown-linux-gnu -p agenthub-acp-adapter --features release-vendored-openssl`
  - `cargo build --locked --release --target aarch64-apple-darwin --bin agenthub --features rocksdb`
  - `cargo build --locked --release --target aarch64-apple-darwin -p agenthub-acp-adapter`
- Confirmed all ACP adapter release legs compiled the renamed internal package:
  `agenthub-codex-acp-runtime v0.10.0`.
- Confirmed the run logs no longer compiled package `agenthub-codex-acp v0.10.0`.
- Confirmed package logs still called only:
  - `package_binary "agenthub"`
  - `package_binary "agenthub-acp"`
- Confirmed the run uploaded the expected matrix artifact bundles:
  - `release-prebuild-x86_64-unknown-linux-gnu`
  - `release-prebuild-aarch64-unknown-linux-gnu`
  - `release-prebuild-aarch64-apple-darwin`

This closes the trimmed `Release Prebuild` runtime package follow-up: push-to-main prebuild now
proves the release matrix builds through the `agenthub-codex-acp-runtime` package identity while
still publishing only the canonical `agenthub` and `agenthub-acp` archive bundles.

The broader partial release validation remains open until a semver or preview release run proves
successful binary assets are published when one release matrix target fails.
