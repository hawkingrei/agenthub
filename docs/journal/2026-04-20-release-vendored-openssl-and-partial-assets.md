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
