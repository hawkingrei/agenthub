## Release OpenSSL Static Linking In Cross Builds

The release build on `main` still depended on OpenSSL discovery through transitive dependencies even though the workspace had already moved its explicit HTTP client usage to `reqwest` with `rustls-tls`.

The remaining OpenSSL dependency does not come from `reqwest`:

- `web-push` defaults to the `isahc-client` path, which brings in `curl`/`curl-sys`;
- `webauthn-rs` pulls in `webauthn-rs-core`, which depends on OpenSSL directly.

The Linux release workflow builds under `cross`, but it previously installed `libssl-dev` only on the host runner. That is not sufficient for the containerized build, so `openssl-sys` failed to locate `openssl.pc`.

The first attempted fix used the `openssl` crate's `vendored` feature. That solved Cargo release builds, but it broke Bazel because `rules_rust` crate-universe did not package the `openssl-src` source tree in the shape that crate expected. The final fix stays on system OpenSSL and follows the documented `openssl` manual configuration path instead.

Per the upstream `openssl` crate docs:

- Unix-like systems use `pkg-config` for automatic discovery;
- `OPENSSL_STATIC` forces static linking;
- target-prefixed or target-scoped environment configuration is appropriate when cross compiling.

The final fix is:

- add `Cross.toml` so Linux `cross` containers install the target-arch `pkg-config`, `libssl-dev`, `libsqlite3-dev`, and `libcap-dev` packages inside the build environment instead of only on the host runner;
- pass `OPENSSL_STATIC=1` into those containerized builds so OpenSSL links statically during release compilation;
- leave Bazel and the workspace dependency graph unchanged.

This keeps the fix constrained to the release path without introducing a new Bazel dependency edge.

Validation for this fix should focus on the release/cross path and Bazel regression risk:

- `cargo check -p agenthub --target-dir /tmp/agenthub-openssl-check` should still pass for the root package;
- release `cross build` jobs should now discover OpenSSL inside the container instead of failing on missing `openssl.pc`;
- Bazel should no longer see any `openssl-src` crate-universe regression because the workspace lockfile and dependency graph are unchanged from the Bazel perspective.
