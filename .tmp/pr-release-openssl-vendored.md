## Summary

- add `Cross.toml` for Linux release targets so `cross` installs target-arch `libssl-dev`, `libsqlite3-dev`, and `libcap-dev` inside the build container
- pass `OPENSSL_STATIC=1` into those Linux `cross` builds, matching the upstream `openssl` crate's documented manual configuration path
- document the release/root-cause analysis in `docs/journal/2026-04-03-release-openssl-cross-static.md`

## Root Cause

The release build still pulled OpenSSL through two transitive paths:

- `web-push` defaults to `isahc`/`curl`
- `webauthn-rs-core` depends on OpenSSL directly

That is fragile for the Linux release workflow because the build runs under `cross`, while `libssl-dev` is installed on the host runner rather than guaranteed inside the build container. The symptom was `openssl-sys` failing to locate `openssl.pc`.

The initial vendored-OpenSSL attempt fixed Cargo release builds, but it introduced a Bazel regression because `rules_rust` crate-universe pulled in `openssl-src`, which does not fit the current Bazel packaging path. The final fix therefore stays on system OpenSSL and limits the change to the release/cross path only.

## Validation

- `cargo check -p agenthub --target-dir /tmp/agenthub-openssl-check2`
- local Bazel verification no longer reached the previous `openssl-src` failure; the remaining local error was an unrelated repository/package-resolution issue under the machine's Bazel environment
