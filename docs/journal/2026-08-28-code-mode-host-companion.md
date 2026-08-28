# Summary

Added the official Codex Code Mode Host as a checksum-pinned release companion while keeping
AgentHub's owned executable surface limited to `agenthub` and `agenthubd`.

# Background

Codex 0.150.1 enables the standalone Code Mode Host provider by default, and Code Mode-only model
metadata cannot fall back to direct tools. Codex resolves `codex-code-mode-host` beside the current
executable. AgentHub packages omitted that companion, so installed Codex workers could not start Code
Mode.

# Scope

- Added a target-aware fetch script for official Codex 0.150.1 Code Mode Host artifacts.
- Pinned and verified the upstream archive digest for each supported AgentHub release target.
- Added the companion to daemon archives, npm platform packages, and Debian packages.
- Kept Cargo and Bazel AgentHub binary targets limited to `agenthub` and `agenthubd`.

# Key Decisions

- Allow `codex-code-mode-host` as an upstream runtime companion, not an AgentHub command or service.
- Keep the V8 runtime out of `agenthubd`. Linking the host library into the daemon would require
  OpenAI's custom V8 archives and bindings in Cargo, cross, and Bazel builds.
- Use the official musl host artifacts for Linux packages, matching OpenAI's portable release
  distribution.
- Couple the companion revision and version to AgentHub's pinned official Codex source and fail closed
  on unsupported targets or checksum mismatches.

# Validation

The official `rust-v0.150.1` annotated tag dereferenced to AgentHub's pinned commit
`90854393966b21e9ebfd21b122334eb09a20c93d`. Local validation downloaded and checksum-verified all
three configured artifacts. The macOS binary exposed the expected `--listen` interface, and `file`
identified the Linux artifacts as statically linked AArch64 and static PIE x86-64 ELF executables.
Two local Bazel attempts remained in Bzlmod loading with zero packages loaded and were interrupted
before analysis; exact-head Bazel CI remains the required terminal proof.

```bash
bash build/codex/fetch-code-mode-host.sh aarch64-apple-darwin /tmp/codex-code-mode-host
/tmp/codex-code-mode-host --help
cargo test --locked -p agenthub --lib release_feature_tests::release_builds_exactly_the_cli_and_daemon_entrypoints -- --nocapture
cargo test --locked -p agenthub-daemon --lib
cargo check --locked -p agenthub-daemon
cargo fmt --all -- --check
bash -n build/codex/fetch-code-mode-host.sh
bash -n build/deb/package.sh
git diff --check
```

# Follow-Ups

- Confirm the exact change head with Bazel CI and Linux cross-release builds.
- Inspect produced tar, npm, and Debian packages and smoke-test the packaged host on each target.
