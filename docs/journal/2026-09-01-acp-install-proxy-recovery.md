# Summary

Moved Codex execution behind an explicit installed-runtime boundary while
keeping AgentHub's built-in Codex ACP worker in the single `agenthubd` daemon
binary. `agenthubd acp codex` now launches the supported official
`codex app-server --stdio` process instead of embedding app-server execution or
acting as Codex's helper multicall binary.

The same change preserves the service-loaded proxy boundary used to recover
provider requests that fall back from WebSockets to HTTPS and then time out.

# Background

AgentHub previously used Codex's in-process app-server client. That made the
daemon responsible for Codex install-context and helper discovery even though
Code Mode and sandbox helpers are part of the official Codex distribution. A
separate packaging workaround downloaded `codex-code-mode-host` into AgentHub
archives, Debian packages, npm packages, and source builds. The ownership split
was fragile: AgentHub could upgrade independently from the helper protocol, and
an otherwise valid AgentHub install could still fail closed because a sibling
helper was missing.

A separate runtime symptom presented as accepted Team or `all` channel messages
without replies. When the provider logged a WebSocket fallback followed by an
HTTPS timeout, durable mailbox delivery was working; the provider child lacked
working service-context egress.

# Scope

- Keep the canonical built-in provider entrypoint as `agenthubd acp codex`.
- Require the official Codex CLI version `0.150.1` and support an absolute
  `[codex_acp].runtime_binary` override.
- Validate the executable and version before serving ACP traffic.
- Spawn `codex app-server --stdio` in each session workspace and bridge its
  JSONL request, response, notification, and server-request traffic.
- Forward the merged ACP-provided MCP server map with app-server thread start
  and resume requests so Team runtime tools remain available.
- Remove AgentHub's Codex argv0 helper dispatch and Code Mode Host download and
  packaging chain.
- Preserve configured proxy environment propagation from the daemon to the ACP
  worker and then to the official Codex child.
- Update stable runtime, distribution, installation, troubleshooting, and
  rollout contracts.

# Key Decisions

- AgentHub owns one daemon executable for server and built-in provider-worker
  modes. The user-facing `agenthub` CLI remains a separate project executable;
  Codex remains an external provider executable.
- Official Codex owns Code Mode Host, sandbox, exec, filesystem, patch,
  authentication, provider transport selection, configuration, and upgrade
  coupling. A WebSocket fallback by itself is not an AgentHub failure; a
  subsequent HTTPS timeout indicates broken provider egress.
- The app-server protocol is treated as version-coupled. AgentHub fails closed
  unless `codex --version` reports `0.150.1`, matching the reviewed protocol
  dependency revision.
- A configured absolute Codex path is preferred for systemd and other managed
  services. Bare `codex` remains the interactive default.
- AgentHub continues to expose ACP as its provider-neutral control-plane
  boundary. Codex app-server JSONL is contained inside the adapter worker.
- Pending Team mailbox evidence is preserved during provider outages. Operators
  repair provider installation or egress before retrying delivery.

# Validation

```bash
cargo fmt --all -- --check
cargo check --locked -p agenthub-codex-acp-runtime -p agenthub-acp-adapter -p agenthub-daemon
cargo test --locked -p agenthub-codex-acp-runtime stdio_app_server --lib -- --nocapture
cargo llvm-cov --locked -p agenthub-codex-acp-runtime --lib --lcov --output-path /tmp/agenthub-codex.lcov -- --test-threads=1
cargo test --locked -p agenthub-acp-adapter --lib -- --nocapture
cargo test --locked -p agenthub-daemon --lib -- --nocapture
cargo test --locked -p agenthub-config codex_runtime_binary -- --nocapture
cargo test --locked -p agenthub --lib release_feature_tests:: -- --nocapture
cargo test --locked -p agenthub local_executor_applies_proxy_policy_to_provider_child -- --nocapture
npm --prefix userdocs run build
bazel test //agenthub-codex-acp:agenthub_codex_acp_tests //crates/agenthub-acp-adapter:agenthub_acp_adapter_tests //crates/agenthub-daemon:agenthub_daemon_tests //:agenthub_unit_tests
git diff --check
```

The focused stdio tests use fake installed Codex executables to verify version
preflight, propagated config and feature arguments, initialize failures and
buffered events, out-of-order typed request correlation, reverse requests,
disconnect propagation, duplicate request IDs, and graceful shutdown without
requiring network access or user credentials. They also verify that child-exit
waits remain bounded when initialization cleanup cannot observe a prompt exit.
Focused thread-parameter coverage verifies that merged ACP MCP servers cross
the external app-server boundary.

## Results

- `cargo fmt --all -- --check` and `git diff --check` passed.
- The focused Cargo check passed for the runtime, adapter, and daemon. A final
  targeted Clippy run with warnings denied also passed for all three packages
  and all targets.
- The Codex ACP runtime library passed all 152 tests. This includes installed
  runtime discovery/version failures, JSONL app-server lifecycle coverage, and
  the merged MCP server process-boundary regression.
- Focused `cargo-llvm-cov` v0.9.0 measurement increased
  `stdio_app_server.rs` line coverage from `452/766` (`59.0%`) to `1176/1270`
  (`92.6%`). New tests correlate responses by JSON-RPC request ID and use
  explicit synchronization for duplicate-ID concurrency instead of relying on
  pipe ordering.
- Review follow-up removed the stale Code Mode companion instruction from the
  production upgrade runbook and made the portable-install contract assert the
  Codex npm package shape without duplicating the version derived from
  `Cargo.lock`. Initialization-failure cleanup now bounds both graceful and
  forced child waits with the shared shutdown timeout.
- The adapter, config, and daemon libraries passed 9, 41, and 4 tests
  respectively. The release-contract suite passed 6 tests, the provider proxy
  propagation regression passed, and the `all` channel mailbox broadcast
  regression passed.
- `cargo metadata --locked --format-version 1 --no-deps` exposed exactly the
  `agenthub` and `agenthubd` binary targets.
- The installed `/usr/bin/codex` completed the real app-server initialize /
  initialized handshake. The final daemon-to-Codex EOF smoke exited
  successfully, while a nonexistent configured path failed before ACP traffic
  with the expected official Codex 0.150.1 diagnostic.
- `npm --prefix userdocs run build` and `bash -n build/deb/package.sh` passed.
- Bazel did not reach target analysis locally. The shared repository cache had
  incomplete `rules_rust`, `bazel_features`, and `rules_cc` entries; each
  reported entry was moved to a recoverable `/tmp` backup and refetched using
  the default Bazel configuration. Module-extension loading then remained
  non-terminal for 257 seconds with zero actions and was interrupted. This
  matches the pre-existing local non-terminal module-resolution evidence in the
  two-binary rollout journal; exact-head Bazel CI remains an open gate.

### Post-Merge Rollout — 2026-09-02

- [PR #1104](https://github.com/hawkingrei/agenthub/pull/1104) merged as
  `4b0b5c7f2e42f2521f2ca04c191ca81fdedc9f67`. Exact-head Release Prebuild run
  `33574882090` produced the Linux amd64 `agenthub` and `agenthubd` artifacts;
  the Debian package contained only those two AgentHub-owned executables.
- The first production ACP smoke exposed a compatibility defect in the
  per-thread MCP override projection. `McpServerConfig.tool_timeout_sec: None`
  serialized as JSON `null`; the official app-server override loader coerced
  that value to an empty string and then rejected it as an invalid `f64`.
- The rollout hotfix omits only a null `tool_timeout_sec` from projected MCP
  server overrides and preserves explicit numeric timeout values. Two focused
  regressions cover both boundaries. The runtime library completed all 153
  tests, and focused Clippy completed with warnings denied.
- The release candidate completed `initialize`, `new_session`, and `prompt`
  against `/usr/bin/codex` 0.150.1. A forced `code_mode_only` prompt launched a
  new `/usr/bin/codex-code-mode-host` process, evaluated JavaScript, returned
  `CODE_MODE_HOST_OK_42`, and ended normally.
- Production was atomically restarted at `2026-09-02 09:41:45 CST`. The service
  reported `active/running`, `Result=success`, and zero automatic restarts.
  Local and public `/health` probes returned HTTP 200. The installed binaries
  have SHA-256 digests
  `9b681afff4abd4210298ecf31720ab40fae2b3c1bc347dc02f63fd696469b1bf`
  (`agenthub`) and
  `da1b604dc3a99fe6ff620c614ad07c7605b4eb971e685dd01929d19d31214646`
  (`agenthubd`).
- Post-install ACP validation returned `INSTALLED_ACP_OK`. Forced Code Mode
  launched a new official host process, returned `INSTALLED_CODE_MODE_OK_42`,
  and ended normally. The pre-hotfix merged binaries remain as `.prev`; the
  pre-#1104 generation remains recoverable as `agenthub.pre-1104` and
  `agenthubd.pre-1104`.
- Exact-head workspace tests completed 799 tests before the `Rust (Coverage)`
  job failed in its separate distributed P2P pipeline step because the
  coverage target directory did not contain `agenthubd`. The dedicated
  Distributed P2P Pipeline job passed, so this workflow-shape failure did not
  block the runtime rollout.

# Follow-Ups

- Keep exact-head release, Debian, npm, and Bazel evidence under the existing
  two-binary rollout item in `docs/todo.md`.
- Reduce the adapter's remaining compile-time use of Codex core/config/history
  types after equivalent app-server endpoints cover all ACP session metadata
  paths. Runtime turn execution is already external in this change.
