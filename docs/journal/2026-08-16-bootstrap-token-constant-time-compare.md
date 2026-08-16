# Summary

`TeamInternalControlService::ensure_bootstrap_token` (flagged in the 2026-08-16 backend correctness
review, tracked in `docs/todo.md`'s Backend Correctness item) compared the caller-supplied bootstrap
token against the configured secret with a plain `!=`. This is the endpoint that mints
cluster-bootstrap credentials for new agent nodes (`issue_node_credential`), so an attacker who can
reach the internal gRPC listener and measure response latency could in principle recover the token
byte-by-byte via a timing side-channel, since `!=` on `&str`/`String` short-circuits at the first
mismatching byte.

# Scope

- `src/internal/service/mod.rs`: added a small `constant_time_eq(a: &[u8], b: &[u8]) -> bool` helper
  (length check first, then an XOR-fold over every byte with no early return) and switched
  `ensure_bootstrap_token` to use it instead of `!=`.

# Key Decisions

- Hand-rolled the comparison instead of adding a dependency (`subtle`, `constant_time_eq`). Both crates
  are already present transitively (via TLS/crypto dependencies) but promoting either to a direct
  dependency of the `agenthub` package requires a `CARGO_BAZEL_REPIN` pass to regenerate
  `MODULE.bazel.lock`, since `crate_universe` resolves this workspace's Bazel crate graph from
  `Cargo.toml`/`Cargo.lock`. For a six-line, well-understood XOR-fold, that Bazel-repin blast radius
  wasn't worth it -- this keeps the fix to a pure Cargo-only, zero-dependency-graph change.
- Length is still checked (and short-circuits) before the fold. This is standard practice for this kind
  of comparison (matching what `subtle`/`constant_time_eq` do too): token length isn't the secret, only
  its content is, so leaking length via a fast path is an accepted tradeoff, not a gap.

# Validation

- `cargo test --lib internal::service` -- 46 passed, including the pre-existing
  `issue_node_credential_rejects_bootstrap_token_mismatch` end-to-end test (still correct: `!=`-shaped
  behavior is preserved, only the comparison strategy changed) and a new
  `constant_time_eq_matches_ordinary_byte_equality` unit test covering equal, empty, differing-length,
  and differing-content-at-various-positions cases.
- `cargo test --lib internal::` -- 69 passed, no regressions.
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- Two findings from the 2026-08-16 backend review remain open in `docs/todo.md`'s Backend Correctness
  item: the remote-relay panic-poisoning `Mutex` trap, and the `context_json` panic landmine in
  `update_team_task`/`run_task_status_sync.rs`. Also still open: the child-process stdin timeout gap and
  the silently-swallowed DB/parse errors noted in the same review.
