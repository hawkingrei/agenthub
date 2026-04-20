# Release Vendored OpenSSL And Partial Assets

## Scope

Align the GitHub release workflow with two concrete requirements:

1. successful targets should still publish binary assets when another target in the release matrix fails;
2. Linux release cross-builds should stop binding to the stale OpenSSL 1.0.2 sysroot that ships in the default `cross` images.

## Changes

- changed `.github/workflows/release.yml` so the `release` job always runs after the build matrix, downloads whatever `release-*` artifacts exist, and only fails when no archives were produced at all;
- generated the release body from the actual `dist/` contents so partial releases no longer advertise assets that were never built;
- removed `libssl-dev` installs from Linux `Cross.toml` pre-build hooks so the cross sysroot no longer injects OpenSSL 1.0.2 into release builds;
- forced a vendored OpenSSL path from the root `Cargo.toml` so transitive Linux release dependencies (`webauthn-rs`, `web-push` / `isahc`, `oauth2` / `reqwest`, and native-tls-adjacent paths) converge on the vendored OpenSSL build instead of probing the cross sysroot.

## Validation

Planned validation for the next release-tag or preview-tag run:

- confirm `Build x86_64-unknown-linux-gnu` and `Build aarch64-unknown-linux-gnu` no longer panic in `openssl-sys`;
- confirm a failing matrix leg no longer suppresses successful release assets from the GitHub Release page;
- record the release workflow run IDs and the resulting release URLs in this note before closing the follow-up TODO item.
