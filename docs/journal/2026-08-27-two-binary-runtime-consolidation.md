# Summary

Consolidated the built-in provider entrypoints into the long-running daemon so official builds expose
two native executables: the user-facing CLI and `agenthubd`.

# Background

The existing release shipped a control-plane executable and a generic ACP adapter, while a legacy
Codex-specific binary target remained buildable. Both adapter entrypoints were per-agent stdio child
processes rather than independently operated services. Their separate files created avoidable install
and version-skew surface.

# Scope

- Added the daemon package and its server/provider/helper dispatch.
- Converted the generic adapter and Codex-specific packages to library-only crates.
- Removed the excluded `cmd/agenthub/Cargo.toml` manifest that redundantly redefined the CLI binary.
- Preserved bare legacy built-in commands through spawn-time rewriting.
- Updated built-in web presets, Cargo, Bazel, release, npm staging, Debian, and systemd surfaces.
- Added the canonical contract in `docs/features/two-binary-runtime.md`.

# Key Decisions

- Keep provider workers as isolated child processes while reusing the daemon file.
- Run Codex argv0 dispatch only for Codex worker/helper invocations, avoiding Codex dotenv and PATH
  side effects during normal daemon startup.
- Pass the sibling CLI path into the Codex runtime explicitly so actor command matching never treats
  the daemon as the CLI.
- Rewrite only exact bare legacy built-in commands; custom paths remain operator authority.
- Count native executable files, not npm shims, service units, maintainer scripts, or external provider
  commands.

# Executable Surface Audit

- Cargo workspace metadata contains only the root `agenthub` target and the `agenthubd` target. The
  excluded `cmd/agenthub/Cargo.toml` duplicate was removed so no standalone manifest can redefine the
  CLI outside the workspace.
- Bazel contains exactly two `rust_binary` rules, matching the Cargo targets. No Go or Python main
  package exists in the tracked source tree.
- `build/npm/agenthub/bin/agenthub.cjs` is a JavaScript launcher for the platform `agenthub` file; it
  does not add a native artifact. Platform packages place `agenthubd` beside that file so sibling
  discovery and provider-worker PATH synthesis remain valid.
- Debian maintainer scripts, `build/deb/package.sh`, and the systemd unit are packaging or service
  entrypoints, not application binaries. The unit executes `agenthubd` directly.
- Gemini, Kimi, Git, shell, and other configured subprocesses remain external dependencies. Codex
  sandbox, exec, filesystem, and patch helper names are aliases or hidden re-execution modes of the
  physical daemon file.

# Validation

```bash
cargo check --locked -p agenthub-daemon -p agenthub
cargo metadata --locked --format-version 1 --no-deps
cargo test --locked -p agenthub-daemon --lib
cargo test --locked -p agenthub-acp-adapter --lib
CARGO_INCREMENTAL=0 cargo test --locked -p agenthub --lib release_feature_tests -- --nocapture
CARGO_INCREMENTAL=0 cargo test --locked -p agenthub --lib daemon_binary::tests -- --nocapture
./node_modules/.bin/vitest run src/agent_presets.test.ts src/use_app_agents.test.tsx \
  src/pages/team/use_team_management_actions.test.tsx \
  src/components/agent_node_detail_shared.test.ts
node --test build/npm/agenthub/tests/launcher.test.cjs
bash -n build/deb/package.sh
npm run build
npm run lint
```

Cargo check passed before the final compatibility-test and documentation cleanup; the final provider
rewrite rebuilt the root test target and its 12 focused tests passed. The daemon library has 4 passing
unit tests, the adapter library has 8, the focused root release and launcher suites have 6, the web
selection has 39, and the npm launcher has 4. The production web build and full web lint pass. Cargo
metadata contains only `agenthub` and `agenthubd`; the source tree contains exactly two Bazel
`rust_binary` rules and no other Rust, Go, or Python native entrypoint. `cargo fmt --all -- --check`,
`git diff --check`, and the Debian shell syntax check pass.

`actionlint` reports no new workflow semantic errors. Its remaining diagnostics reproduce on the base
workflow: the prebuild script interpolates `github.head_ref` directly, and existing shell snippets
trigger shellcheck findings.

`CARGO_BAZEL_REPIN=true bazel mod deps` did not reach a terminal result in the local environment. It
remained in external module graph resolution after reporting the repository's existing direct-version
warnings for `bazel_skylib` and `rules_cc`, and was interrupted without changing `MODULE.bazel.lock`.

# Follow-Ups

- Validate the exact change head through Bazel CI; local module resolution was non-terminal.
- Prove Linux cross-builds and inspect generated Debian and npm platform packages for exactly the two
  expected native files.
- Update third-party installation guidance that still assumes a separately installed adapter.
