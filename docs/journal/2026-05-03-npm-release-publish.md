# Summary

Added the first release-time npm distribution path for the `agenthub` binary under the
`@linkerdog` scope.

# Background

The repository already produced Rust release archives, but there was no supported npm install
path for operators who want `npm install -g @linkerdog/agenthub`.

The existing `web` and `userdocs` packages are private application packages, so they are not
appropriate npm publish targets. The correct distribution unit is the Rust CLI binary itself.

# Scope

- add scoped npm package skeletons for the wrapper package and supported platform packages
- add release workflow steps that publish these packages to npmjs using `NPM_TOKEN`
- keep GitHub release archives as the existing distribution channel
- keep the package skeletons under `build/npm/` so the repo layout matches their release-only role

# Key Decisions

- use one wrapper package plus platform-specific optional dependency packages instead of shipping
  all binaries in a single npm tarball
- publish only on semver-compatible release versions
- keep the first supported npm platforms aligned with the current Rust release build matrix:
  `darwin/arm64`, `linux/arm64`, `linux/x64`
- set the in-repo npm package skeleton version baseline to `0.0.3`; the release workflow still
  derives the actual publish version from the semver tag without the leading `v`

# Validation

- `node --test build/npm/agenthub/tests/launcher.test.cjs`
- `cd build/npm/agenthub && npm pack --dry-run`
- `gh run view 29194967848 --log | rg "npm publish|@linkerdog/agenthub|NPM_TOKEN|NODE_AUTH_TOKEN|Publishing"`
  - release run `29194967848` used `NODE_AUTH_TOKEN` from the release environment
  - platform packages published first:
    - `@linkerdog/agenthub-darwin-arm64@0.0.11`
    - `@linkerdog/agenthub-linux-arm64@0.0.11`
    - `@linkerdog/agenthub-linux-x64@0.0.11`
  - wrapper package published after the platform packages:
    - `@linkerdog/agenthub@0.0.11`
- `npm view @linkerdog/agenthub version dist-tags time versions --json --cache /private/tmp/agenthub-npm-cache`
  - latest wrapper package is `0.0.11`
  - `0.0.11` was published at `2026-07-12T15:04:58.611Z`
- `npm view @linkerdog/agenthub-darwin-arm64 version dist-tags time --json --cache /private/tmp/agenthub-npm-cache`
  - latest platform package is `0.0.11`
  - `0.0.11` was published at `2026-07-12T15:04:15.728Z`
- `npm view @linkerdog/agenthub-linux-arm64 version dist-tags time --json --cache /private/tmp/agenthub-npm-cache`
  - latest platform package is `0.0.11`
  - `0.0.11` was published at `2026-07-12T15:04:35.498Z`
- `npm view @linkerdog/agenthub-linux-x64 version dist-tags time --json --cache /private/tmp/agenthub-npm-cache`
  - latest platform package is `0.0.11`
  - `0.0.11` was published at `2026-07-12T15:04:57.338Z`

# Published Package URLs

- `https://www.npmjs.com/package/@linkerdog/agenthub/v/0.0.11`
- `https://www.npmjs.com/package/@linkerdog/agenthub-darwin-arm64/v/0.0.11`
- `https://www.npmjs.com/package/@linkerdog/agenthub-linux-arm64/v/0.0.11`
- `https://www.npmjs.com/package/@linkerdog/agenthub-linux-x64/v/0.0.11`

## 2026-07-28 Live Publish Verification

- Observed successful `Release` run `29194967848` for tag `v0.0.11`.
- Confirmed the publish job had `NPM_TOKEN` present and published platform packages before the
  wrapper package:
  - `@linkerdog/agenthub-darwin-arm64@0.0.11`
  - `@linkerdog/agenthub-linux-arm64@0.0.11`
  - `@linkerdog/agenthub-linux-x64@0.0.11`
  - `@linkerdog/agenthub@0.0.11`
- Confirmed npm registry state reports `latest: 0.0.11` for the wrapper package and Linux platform
  packages.

Validation:

- `gh run view 29194967848 --log | rg -n "npm publish|@linkerdog/agenthub"`
- `npm --cache /private/tmp/npm-cache-agenthub view @linkerdog/agenthub version dist-tags time repository --json`
- `npm --cache /private/tmp/npm-cache-agenthub view @linkerdog/agenthub-linux-x64 version dist-tags time --json`
- `npm --cache /private/tmp/npm-cache-agenthub view @linkerdog/agenthub-linux-arm64 version dist-tags time --json`

# Follow-Ups

- add additional platform packages if release coverage expands beyond the current three targets
