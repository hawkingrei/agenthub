# Summary

Closed two operational gaps that could leave a Codex-backed AgentHub runtime
partially functional: portable/source installs now stage the pinned Code Mode
Host beside `agenthubd`, and provider timeout recovery now points operators to
the service-loaded AgentHub proxy configuration instead of mailbox mutation.

# Background

The release pipeline already placed the checksum-pinned
`codex-code-mode-host` in daemon archives, Debian packages, and npm platform
packages. The portable archive instructions copied only `agenthub` and
`agenthubd`, however, and `make run` built the daemon without staging its
companion. Both paths made Codex resolve a missing sibling host.

A separate failure presented as accepted Team messages without replies. The
mailbox fan-out remained durable, but the provider logged a WebSocket fallback
followed by an HTTPS timeout because the service context had no working egress
configuration. Editing mailbox rows cannot repair that boundary.

# Scope

- Stage the pinned official Code Mode Host before supported Makefile build and
  run paths.
- Install and verify the companion in portable archive instructions and align
  package, upgrade, uninstall, and production-checklist wording.
- Document the exact Code Mode missing-host and provider-timeout signatures.
- Define `[proxy]` as the preferred provider-egress boundary for
  systemd-managed local runtimes.
- Add focused Rust contract tests for source/portable staging and proxy
  environment expansion.

# Key Decisions

- Keep V8 isolated in the official companion process; do not link the Code
  Mode runtime into `agenthubd`.
- Reuse the existing checksum-pinned fetch script for source development so
  release and local workflows follow the same Codex revision.
- Require the companion to remain beside `agenthubd`; do not add a
  version-ambiguous PATH fallback.
- Prefer AgentHub TOML proxy configuration over interactive-shell exports for
  managed services. AgentHub expands configured values into upper- and
  lower-case variables for provider children.
- Preserve pending Team mailbox evidence during provider outages and repair
  egress before retrying delivery.

# Validation

```bash
make -n build-code-mode-host
make build-code-mode-host
target/debug/codex-code-mode-host --help
cargo test --locked -p agenthub-config proxy_config_expands_for_service_managed_provider_children -- --nocapture
cargo test --locked -p agenthub local_executor_applies_proxy_policy_to_provider_child -- --nocapture
cargo test --locked -p agenthub --lib source_and_portable_installs_stage_the_code_mode_host -- --nocapture
cargo test --locked -p agenthub --lib provider_timeout_recovery_uses_service_loaded_proxy_config -- --nocapture
cargo fmt --all -- --check
npm --prefix userdocs run build
bazel test //:agenthub_unit_tests --test_filter=release_feature_tests:: --test_output=errors
git diff --check
```

The focused Cargo checks, companion download/checksum/CLI smoke, formatting,
and user-documentation build form the local acceptance set. The default Bazel
attempt stopped during external `rules_rust` package loading before analysis
because `@rules_rust//rust:defs.bzl` had no enclosing BUILD package in the
resolved repository; exact-head Bazel CI remains the terminal proof.

# Follow-Ups

- Keep published archive, Debian, and npm companion inspection under the
  existing two-binary rollout evidence item in `docs/todo.md`, including the
  exact-head Bazel check that the local repository-loading failure could not
  reach.
