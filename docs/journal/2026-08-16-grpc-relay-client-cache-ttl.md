# Summary

`TeamRemoteRelayAdapter.grpc_client_cache` (flagged in the 2026-08-16 backend correctness review,
tracked in `docs/todo.md`'s Backend Correctness item) never evicted anything. For the common
"registered" gRPC relay path, the cache key includes a freshly-minted access token stamped with the
current timestamp on every relayed message, so almost every delivery inserted a brand-new entry that
was looked up exactly once and then kept forever -- an unbounded, per-message leak of live gRPC
`Channel`s, not a small admin-bounded one. Separately, callers that *do* reuse a stable, explicit
`access_token` in their route got real cache hits, but the cached `InternalGrpcMailboxClient` pins the
token it was built with and never refreshes it, so a long-lived cache entry could silently start
sending an expired bearer token once the token's TTL elapsed.

# Background

`grpc_client_for_route` (`src/team/manager/remote_relay_grpc.rs`) is the sole reader/writer of the
cache, called once per relayed message from `deliver_grpc`. The only existing eviction was a blunt
full-`clear()` in `configure_grpc_tls_defaults`, triggered only when cluster-wide gRPC TLS defaults are
reconfigured -- unrelated to normal cache growth or token freshness. `access_token`, part of the cache
key (`GrpcRelayClientCacheKey`), is minted per call via `issue_registered_grpc_access_token` with a
600s TTL and an embedded `iat`/`exp`, so the key is effectively unique per delivery on that path.

# Scope

- `src/team/manager/remote_relay_types.rs`: cache value changed from `InternalGrpcMailboxClient` to
  `CachedGrpcRelayClient { client, inserted_at: Instant }`. New `RELAY_GRPC_CLIENT_CACHE_TTL` constant
  (240s -- kept below the 600s minted-token TTL so a cached client is dropped, and a fresh token
  obtained, before the token it holds expires).
- `src/team/manager/remote_relay_grpc.rs`: `grpc_client_for_route` now sweeps (`HashMap::retain`) every
  entry older than the TTL on each call, before checking for a cache hit, then inserts new entries with
  the current `Instant`. This runs on both hits and misses, so the cache is bounded even under a
  never-hits key pattern (the common per-message-token case) -- it never grows past roughly one TTL
  window's worth of entries, instead of growing forever.
- `src/internal/client/mod.rs`: added a `#[cfg(test)]`-only `InternalGrpcMailboxClient::test_stub`
  constructor (a `Channel` built via `connect_lazy()`, no real I/O) so tests can populate synthetic
  cache entries without standing up a live gRPC server.

# Key Decisions

- TTL-based lazy eviction, not a size cap/LRU: an LRU bounds memory but doesn't address the token-
  staleness correctness gap (a cached client with an expired token would keep being served and start
  failing auth). A fixed TTL close to the minted-token lifetime fixes both the leak and the staleness
  issue in one change, confirmed with the person requesting this fix.
- Eviction is lazy (sweep-on-call), not a background task: this is a request-driven cache with no
  existing periodic-task infrastructure in this adapter; sweeping on every call (hit or miss) already
  bounds growth without needing a new timer/task lifecycle.
- Left the full-`clear()` on TLS-default reconfiguration in place -- orthogonal to TTL expiry and still
  correct (any client built against now-stale TLS material should be dropped immediately, not wait out
  the TTL).

# Validation

- `cargo test --lib team::manager::remote_relay` -- 17 passed (2 new:
  `grpc_client_for_route_returns_cached_client_before_ttl_expires`,
  `grpc_client_for_route_evicts_expired_entries_and_reconnects`). Both use `test_stub` clients and a
  deliberately unconnectable `target` string, so an `Ok` result can only mean a cache hit and an `Err`
  containing "gRPC connect failed" can only mean a real (failing) reconnect was attempted --
  distinguishing "served from cache" from "evicted and rebuilt" without needing a live gRPC server.
- `cargo test --lib team::manager` (183 passed) and `cargo test --lib internal::client` (15 passed) --
  no regressions in either module the change touches.
- `cargo test --lib` -- 764 passed, 3 pre-existing failures in `state::tests::*`
  (`lance-namespace-impls` panic, "the directory manifest dataset must not enable old-version
  cleanup") confirmed present on `main` before this change via `git stash`; unrelated to this fix.
- `cargo clippy --lib --tests -p agenthub` and `cargo fmt -p agenthub -- --check` clean.

# Follow-Ups

- The other three findings from the 2026-08-16 backend review (remote-relay panic-poisoning `Mutex`
  trap, `context_json` panic landmine, bootstrap-token timing side-channel, stdin timeout gap, silent
  DB/parse errors) remain open in `docs/todo.md`'s Backend Correctness item.
