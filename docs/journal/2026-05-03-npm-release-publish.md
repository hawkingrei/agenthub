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

# Follow-Ups

- add additional platform packages if release coverage expands beyond the current three targets
- verify the `@linkerdog` npm scope and `NPM_TOKEN` publish permissions before the first live
  semver release
