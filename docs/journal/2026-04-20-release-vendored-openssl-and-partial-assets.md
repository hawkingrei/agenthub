# Release Vendored OpenSSL And Partial Assets

## Scope

Align the GitHub release workflow with two concrete requirements:

1. successful targets should still publish binary assets when another target in the release matrix fails;
2. Linux release cross-builds should stop binding to the stale OpenSSL 1.0.2 sysroot that ships in the default `cross` images.

## Changes

- changed `.github/workflows/release.yml` so the `release` job always runs after the build matrix, downloads whatever `release-*` artifacts exist, and only fails when no archives were produced at all;
- generated the release body from the actual `dist/` contents so partial releases no longer advertise assets that were never built;
- added `.github/workflows/release-prebuild.yml` so merges to `main` exercise the same release-target matrix and packaging path before a tag is cut;
- removed `libssl-dev` installs from Linux `Cross.toml` pre-build hooks and explicitly installed `zlib1g-dev` so the cross sysroot no longer injects OpenSSL 1.0.2 into release builds while still keeping compression headers available;
- moved vendored OpenSSL behind release-only Cargo features (`release-vendored-openssl`) in both `agenthub` and the ACP adapter stack so Linux release builds still converge on vendored OpenSSL without dragging `openssl-src` into the default Bazel / crate_universe dependency graph.
- kept vendored OpenSSL scoped to Linux cross release/prebuild legs only, excluded source-only archives from the release artifact guard, and removed `continue-on-error` from artifact download so infrastructure failures still stop the release job;
- added a Linux `memfd_create` compatibility shim in `agenthub-codex-acp` so V8-backed release/prebuild links no longer depend on the older GNU cross sysroot exporting that libc wrapper.

## Validation

### 2026-06-11 Release Prebuild Verification

- Observed main `Release Prebuild` run `27359976172` after merge commit `6e64fe95e1517895a23686de0f5d1be34136ad35`.
- Confirmed the workflow triggers on `push` to `main` and starts the same release-target matrix:
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and `aarch64-apple-darwin`.
- Confirmed the run completed successfully:
  - `Prebuild x86_64-unknown-linux-gnu`: success, 1h12m13s.
  - `Prebuild aarch64-unknown-linux-gnu`: success, 1h20m49s.
  - `Prebuild aarch64-apple-darwin`: success, 38m50s.
- Confirmed both Linux `agenthub` and `agenthub-acp` cross builds passed with
  `release-vendored-openssl`; the stale cross-sysroot OpenSSL failure did not recur for the primary
  release binary or canonical ACP adapter.
- Downloaded the produced artifacts and confirmed the current workflow still emits:
  - `agenthub-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
  - `agenthub-acp-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
  - `agenthub-codex-acp-main-{linux-amd64,linux-arm64,darwin-arm64}.tar.gz`.
- Found that the prebuild still spent release time building and packaging the legacy
  `agenthub-codex-acp` binary after `agenthub-acp` became the canonical ACP release entrypoint.
  The follow-up change removes `agenthub-codex-acp` from release and prebuild package outputs while
  leaving compatibility detection and internal adapter reuse intact.
- Found a non-fatal `dtolnay/rust-toolchain` annotation for the unsupported `profile` input. The
  follow-up change removes that input from both release workflows.

Planned validation for the next release-tag or preview-tag run:

### 2026-07-12 Semver Release Verification

- Observed semver release run `29194967848` for tag `v0.0.11` at commit
  `53edfc9cb8bb3f73b2efa9807e8462fcc664edd7`.
- Confirmed all build-matrix jobs completed successfully:
  - `Build x86_64-unknown-linux-gnu`
  - `Build aarch64-unknown-linux-gnu`
  - `Build aarch64-apple-darwin`
- Confirmed both Linux release legs used `release-vendored-openssl`:
  - `cross build --locked --release --target x86_64-unknown-linux-gnu --bin agenthub --features release-vendored-openssl,release-lance-fp16,rocksdb`
  - `cross build --locked --release --target x86_64-unknown-linux-gnu -p agenthub-acp-adapter --features release-vendored-openssl`
  - `cross build --locked --release --target aarch64-unknown-linux-gnu --bin agenthub --features release-vendored-openssl,rocksdb`
  - `cross build --locked --release --target aarch64-unknown-linux-gnu -p agenthub-acp-adapter --features release-vendored-openssl`
- Confirmed the stale cross-sysroot OpenSSL panic did not recur; `openssl-sys v0.9.117` compiled in
  both Linux release legs.
- Confirmed release `v0.0.11` published Linux and macOS assets for both canonical binaries:
  - `agenthub-0.0.11-darwin-arm64.tar.gz`
  - `agenthub-0.0.11-linux-amd64.tar.gz`
  - `agenthub-0.0.11-linux-arm64.tar.gz`
  - `agenthub-acp-0.0.11-darwin-arm64.tar.gz`
  - `agenthub-acp-0.0.11-linux-amd64.tar.gz`
  - `agenthub-acp-0.0.11-linux-arm64.tar.gz`
- Confirmed release `v0.0.11` did not publish `agenthub-codex-acp` release assets.
- Confirmed the semver release completed successfully, so it does not by itself prove partial-asset
  behavior under a failing matrix leg.

Remaining validation before closing the release follow-up TODO:

- verify a preview release run publishes successful binary assets even if one release matrix target
  fails;
- record the preview release workflow run ID and release URL.

### 2026-07-18 Release Prebuild Trim Check

- Observed `Release Prebuild` run `29639782865` on `main` after PR `#890`.
- Confirmed the current workflow package script calls only:
  - `package_binary "agenthub"`
  - `package_binary "agenthub-acp"`
- Confirmed the run uploaded the expected prebuild artifact bundles:
  - `release-prebuild-x86_64-unknown-linux-gnu`
  - `release-prebuild-aarch64-unknown-linux-gnu`
  - `release-prebuild-aarch64-apple-darwin`
- Did not close the trimmed prebuild follow-up because the run log still shows
  `agenthub-codex-acp v0.10.0` being compiled in release/prebuild legs through the ACP adapter
  dependency path. The final published archives are trimmed, but the actual release-build scope is
  not yet proven to be trimmed to only `agenthub` and `agenthub-acp`.

### 2026-07-19 Local Dependency Path Check And Runtime Boundary Split

- Confirmed the remaining build-scope leak is structural rather than a packaging-script-only issue:
  `crates/agenthub-acp-adapter/Cargo.toml` still has a direct dependency on
  `agenthub-codex-acp`, and `crates/agenthub-acp-adapter/src/lib.rs` dispatches
  `agenthub-acp codex` to `agenthub_codex_acp::run_main(...)`.
- Split the Cargo package/library boundary without moving the implementation files:
  - the package is now `agenthub-codex-acp-runtime`;
  - the library crate is now `agenthub_codex_acp_runtime`;
  - the compatibility binary target remains `agenthub-codex-acp`;
  - `agenthub-acp-adapter` now depends on `agenthub-codex-acp-runtime` for the Codex provider.
- Kept the trimmed prebuild follow-up open because local manifest checks cannot replace a real
  release/prebuild run. The next validation step is a `Release Prebuild` run whose logs no longer
  compile package `agenthub-codex-acp`.
- Added a local release-feature guard so `src/lib.rs` rejects a regression back to the legacy
  `agenthub-acp-adapter -> agenthub-codex-acp` package dependency while still requiring real
  release/prebuild evidence before the TODO closes.
- Extended the same local guard to reject release and release-prebuild workflow regressions that
  package the legacy `agenthub-codex-acp` compatibility binary instead of only the canonical
  `agenthub` and `agenthub-acp` binary archives.
- Updated active local build and feature-spec validation commands to use
  `agenthub-codex-acp-runtime`, with an explicit compatibility binary build for
  `agenthub-codex-acp`.

Focused validation for this local check:

```bash
cargo test release_prebuild_trim_todo_stays_open_until_real_prebuild_proves_legacy_crate_is_gone --lib
cargo test -p agenthub-codex-acp-runtime resolve_agenthub_codex_acp_otel_enabled_defaults_to_false
cargo test -p agenthub-acp-adapter maps_codex_config_overrides
cargo build -p agenthub-codex-acp-runtime --bin agenthub-codex-acp
cargo metadata --no-deps --format-version 1
bazel query //agenthub-codex-acp:agenthub_codex_acp_runtime
bazel query //crates/agenthub-acp-adapter:agenthub_acp_adapter
```

Local Bazel build caveat:

```bash
bazel build //crates/agenthub-acp-adapter:agenthub_acp_adapter_bin //agenthub-codex-acp:agenthub_codex_acp_bin
```

On the local macOS host this reached the new
`//agenthub-codex-acp:agenthub_codex_acp_runtime` dependency path but failed during analysis because
the existing Codex `codex-linux-sandbox` transitive dependency requires the Linux-only
`//third_party/codex_linux_sandbox:vendored_bwrap_ffi` target. This does not prove release/prebuild
success; the authoritative closure evidence remains a real `Release Prebuild` run.

### 2026-07-19 External Workflow Evidence Audit

- Checked GitHub Actions workflow inventory and confirmed the relevant active workflows are still
  `Release Prebuild` (`.github/workflows/release-prebuild.yml`) and `Release`
  (`.github/workflows/release.yml`).
- Checked the latest `Release Prebuild` runs. The newest run remains `29639782865`, created
  `2026-07-18T09:48:34Z`, before the local `agenthub-codex-acp-runtime` split had external
  workflow evidence.
- Re-read run `29639782865` logs and confirmed they still compile package
  `agenthub-codex-acp v0.10.0` in all release/prebuild legs while packaging only `agenthub` and
  `agenthub-acp`.
- Checked the latest `Release` workflow runs. The newest semver release run remains `29194967848`
  for `v0.0.11`; it completed with every matrix leg successful, so it still does not prove preview
  partial-asset behavior under a failing release target.
- Refreshed the workflow inventory on 2026-07-19 at 11:15 UTC. The newest `Release Prebuild` run
  was still `29639782865`, and the newest `Release` run was still `29194967848`.
- Kept both release TODOs open. Closing them still requires:
  - a newer `Release Prebuild` run whose logs prove the build scope no longer compiles package
    `agenthub-codex-acp`; and
  - a preview release run proving successful binary assets publish when at least one release matrix
    target fails.

### 2026-07-19 Partial Release Historical Failure Audit

- Refreshed the workflow inventory on 2026-07-19 at 12:42 UTC. The newest `Release` run remained
  `29194967848` for tag `v0.0.11`, and the newest `Release Prebuild` run remained `29639782865`.
- Checked historical failed release run `25605747548` for tag `v0.0.3` as a possible partial-asset
  proof.
- Found that successful build jobs in that run uploaded workflow artifacts for
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, and `aarch64-apple-darwin`, but the
  `Publish npm packages` job failed and the dependent `Create Release` job was skipped.
- Checked release `v0.0.3` and confirmed its GitHub release asset list is empty.
- Kept the partial-asset release TODO open because `v0.0.3` proves only that build artifacts can
  exist inside a failed workflow run; it does not prove that a release publishes successful binary
  assets when another release matrix target fails.

### 2026-07-19 Follow-Up Workflow Inventory Audit

- Refreshed the workflow inventory on 2026-07-19 at 13:23 UTC.
- The newest `Release` run remained `29194967848` for tag `v0.0.11`; it is still a fully successful
  semver release and still does not prove partial release publication under a failing matrix leg.
- The newest `Release Prebuild` run remained `29639782865`; it still predates external evidence for
  the local `agenthub-codex-acp-runtime` package split.
- Kept both release TODOs open because there was no newer workflow evidence to inspect.

### 2026-07-19 13:44 UTC Workflow Inventory Audit

- Refreshed the workflow inventory again on 2026-07-19 at 13:44 UTC.
- The newest `Release` run remained `29194967848` for tag `v0.0.11`; it is still a fully successful
  semver release and still does not prove partial release publication under a failing matrix leg.
- The newest `Release Prebuild` run remained `29639782865`, created
  `2026-07-18T09:48:34Z`.
- Re-read run `29639782865` logs and confirmed it still compiles package
  `agenthub-codex-acp v0.10.0` in release/prebuild legs while packaging only `agenthub` and
  `agenthub-acp`.
- Kept both release TODOs open because there was no newer workflow evidence after the local runtime
  package split and no preview release run demonstrating partial binary publication.

### 2026-07-19 Local Partial-Asset Workflow Guard

- Added a local release workflow structure guard so `src/lib.rs` fails if the release matrix
  re-enables `fail-fast`, if the `Create Release` job stops using `always()`, if it requires a fully
  successful build matrix before collecting artifacts, if it stops downloading `release-*`
  artifacts with `merge-multiple`, or if it drops the fail-closed check for zero binary assets.
- The same guard requires the release body to keep the partial-build warning and requires the active
  TODO to remain open until a real preview release run proves successful binary assets publish when
  one matrix target fails.
- Kept the partial-asset TODO open because this is a static workflow guard; it does not provide the
  required live preview release evidence.

Focused validation for this external audit:

```bash
gh run list --workflow release-prebuild.yml --limit 20 --json databaseId,displayTitle,event,headBranch,status,conclusion,createdAt,updatedAt,url
gh run list --workflow release.yml --limit 20 --json databaseId,displayTitle,event,headBranch,status,conclusion,createdAt,updatedAt,url
gh run view 29639782865 --log | rg -n "agenthub-codex-acp v|agenthub-codex-acp-runtime|package_binary|release-prebuild-|Uploading artifact|Artifact name|cross build|cargo build|failed|error"
gh run view 29194967848 --log | rg -n "agenthub-codex-acp|agenthub-acp-|agenthub-|Uploading artifact|Artifact name|failed|failure|cancelled|cross build|gh release|softprops|release"
gh run view 25605747548 --repo hawkingrei/agenthub --json databaseId,name,event,headBranch,headSha,status,conclusion,createdAt,updatedAt,jobs,url
gh release view v0.0.3 --repo hawkingrei/agenthub --json tagName,name,publishedAt,isDraft,isPrerelease,assets,url
cargo test release_workflow_keeps_partial_asset_publication_path_open --lib -- --nocapture
cargo test release_prebuild_trim_todo_stays_open_until_real_prebuild_proves_legacy_crate_is_gone --lib -- --nocapture
```
