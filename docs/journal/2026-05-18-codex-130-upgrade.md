# Summary

Upgraded `agenthub-codex-acp` from the official `openai/codex` `rust-v0.129.0` baseline to
`rust-v0.130.0` (release commit `58573da43ab697e8b79f152c53df4b42230395a8`).

The AgentHub ACP adapter only needed a narrow compatibility update for the new
`started_at_ms` field on approval-related protocol events.

# Background

AgentHub keeps the ACP adapter close to the upstream Codex release train so the
app-server bridge does not accumulate avoidable protocol drift.

Current `main` already uses Cargo tag pins for the direct Codex Rust crates while
keeping `MODULE.bazel`'s `codex_src` fetch separate. This upgrade follows that
current repository shape instead of reintroducing an older "pin everything to one
rev" layout.

# Scope

- bumped all direct `agenthub-codex-acp` Codex git dependencies from
  `rust-v0.129.0` to `rust-v0.130.0`
- refreshed `Cargo.lock` onto the `0.130.0` Codex graph
- updated ACP approval event translation and test fixtures for the newly required
  `started_at_ms` field

# Key Decisions

- Keep the upgrade narrow.
  The first `0.130.0` compile break was limited to approval event schema changes,
  so the adapter now forwards `started_at_ms` from app-server approval params into
  ACP-facing protocol events and otherwise keeps behavior unchanged.

- Do not change `MODULE.bazel` in this slice.
  The current repository no longer ties Bazel's `codex_src` fetch directly to the
  Cargo tag pin, and the focused Rust ACP validation passed without any Bazel-side
  source adjustment.

- Treat test fixtures as part of the compatibility surface.
  `cargo check --tests` surfaced the same schema change in ACP test-only event
  constructors, so those fixtures now set `started_at_ms: 0` explicitly.

# Validation

Executed:

```bash
TMPDIR=$PWD/.tmp \
CARGO_HOME=$PWD/.cargo-home-temp \
CARGO_NET_GIT_FETCH_WITH_CLI=true \
RUSTC=/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc \
RUSTDOC=/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustdoc \
/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo check -p agenthub-codex-acp
```

```bash
TMPDIR=$PWD/.tmp \
CARGO_HOME=$PWD/.cargo-home-temp \
CARGO_NET_GIT_FETCH_WITH_CLI=true \
RUSTC=/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc \
RUSTDOC=/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustdoc \
/home/hawkingrei/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/cargo check -p agenthub-codex-acp --tests
```

Result:

- both focused ACP checks passed on the `rust-v0.130.0` dependency graph
- the adapter compiles after forwarding `started_at_ms` through approval request
  translations
- the ACP test target also compiles after updating test-only approval fixtures

# Follow-Ups

None for the Rust ACP adapter slice in this change.
