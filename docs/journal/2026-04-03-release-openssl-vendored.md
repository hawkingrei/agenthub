## Release OpenSSL Vendoring

The release build on `main` still depended on system OpenSSL discovery even though the workspace had already moved its explicit HTTP client usage to `reqwest` with `rustls-tls`.

The remaining OpenSSL dependency did not come from `reqwest`:

- `web-push` defaults to the `isahc-client` path, which brings in `curl`/`curl-sys`;
- `webauthn-rs` pulls in `webauthn-rs-core`, which depends on OpenSSL directly.

That combination is fragile for the release workflow because the Linux release builds run under `cross`, while `libssl-dev` is installed on the host runner rather than guaranteed inside the build container. The symptom is `openssl-sys` failing to locate `openssl.pc`.

The fix is intentionally small:

- add an explicit root dependency on `openssl` with the `vendored` feature enabled;
- rely on Cargo feature unification so the entire `openssl` / `openssl-sys` graph builds against vendored OpenSSL instead of requiring a system installation.

This keeps the release path hermetic across native and cross builds without changing the auth or push-notification behavior.

Validation for this fix should focus on dependency shape and the root package build:

- `cargo tree -e features -i openssl-sys` should show the vendored feature enabled through the unified graph;
- `cargo check -p agenthub` should continue to compile the root package successfully;
- the release workflow should no longer require `pkg-config` to locate a host/container OpenSSL installation for the Rust build itself.
