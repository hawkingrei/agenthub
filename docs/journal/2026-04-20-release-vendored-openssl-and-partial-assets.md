# Release Vendored OpenSSL And Partial Assets

## Scope

Align the GitHub release workflow with two concrete requirements:

1. successful targets should still publish binary assets when another target in the release matrix fails;
2. Linux release cross-builds should stop binding to the stale OpenSSL 1.0.2 sysroot that ships in the default `cross` images.

## Changes

- changed `.github/workflows/release.yml` so the `release` job always runs after the build matrix, downloads whatever `release-*` artifacts exist, and only fails when no archives were produced at all;
- generated the release body from the actual `dist/` contents so partial releases no longer advertise assets that were never built;
- added `.github/workflows/release-prebuild.yml` so normal `push` and `pull_request` traffic now exercises the same release-target matrix and packaging path before a tag is cut;
- removed `libssl-dev` installs from Linux `Cross.toml` pre-build hooks and explicitly installed `zlib1g-dev` so the cross sysroot no longer injects OpenSSL 1.0.2 into release builds while still keeping compression headers available;
- moved vendored OpenSSL behind release-only Cargo features (`release-vendored-openssl`) in both `agenthub` and `agenthub-codex-acp` so Linux release builds still converge on vendored OpenSSL without dragging `openssl-src` into the default Bazel / crate_universe dependency graph.

## Validation

Planned validation for the next release-tag or preview-tag run:

- confirm `Build x86_64-unknown-linux-gnu` and `Build aarch64-unknown-linux-gnu` no longer panic in `openssl-sys`;
- confirm a failing matrix leg no longer suppresses successful release assets from the GitHub Release page;
- confirm `Release Prebuild` catches the same cross-build regressions on both `push` and `pull_request` before a tag is cut;
- record the release workflow run IDs and the resulting release URLs in this note before closing the follow-up TODO item.
