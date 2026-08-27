# Debian Systemd Distribution

## Problem

GitHub release archives provide portable Linux binaries, but they do not create a managed service.
Operators who deploy AgentHub on Debian or Ubuntu hosts need a package that installs the binaries,
creates a stable runtime identity, and starts AgentHub through systemd by default.

## Scope

- Debian package assets for Linux `amd64` and `arm64` release targets.
- Installation of both `agenthub` and `agenthubd`.
- A default `agenthub.service` systemd unit.
- Package maintainer scripts that create the service user, create the runtime layout, and enable/start
  the service on install.
- Release and release-prebuild workflow coverage for Debian package generation.

## Non-Goals

- Replacing GitHub `.tar.gz` archives.
- Replacing npm or Homebrew distribution.
- Supporting Windows, RPM, Snap, or Docker packages in this slice.
- Managing reverse proxy, TLS certificates, or public hostname setup.
- Deleting runtime data automatically on package removal or purge.

## Architecture

Linux release jobs build the existing release binaries and then run `build/deb/package.sh`.
The package script stages a Debian package root and invokes `dpkg-deb --build --root-owner-group`.

The package installs:

- `/usr/bin/agenthub`
- `/usr/bin/agenthubd`
- `/usr/lib/systemd/system/agenthub.service`
- `/usr/share/doc/agenthub/README.md`

The service runs as the `agenthub` system user with:

- `HOME=/var/lib/agenthub`
- `WorkingDirectory=/var/lib/agenthub`
- `ExecStart=/usr/bin/agenthubd`

The existing AgentHub config lookup therefore resolves to
`/var/lib/agenthub/.agenthub/config.toml` without adding a new config-path CLI flag.

## Contracts

### Package Assets

Release assets include Debian packages for Linux targets:

- `agenthub_<version>_amd64.deb`
- `agenthub_<version>_arm64.deb`

Preview or branch prebuild versions that are not valid semver are normalized to a Debian-safe
`0.0.0+<name>` version inside the package metadata.

### Runtime User And Data

The package creates an `agenthub` system group and user when they do not already exist. Runtime data
is rooted at `/var/lib/agenthub` and is preserved on remove and purge.

### Default Service

On install, the package attempts to:

1. reload systemd units;
2. enable `agenthub.service`;
3. start or restart `agenthub.service`.

If systemd is unavailable or service startup fails, package installation still completes and leaves
diagnostics in stderr.

### Default Config

If no service config exists, package install writes
`/var/lib/agenthub/.agenthub/config.toml` with:

- `server.listen = "127.0.0.1:8080"`
- `worktree.default_root = "/var/lib/agenthub/.agenthub/worktrees"`
- message archive/body paths under `/var/lib/agenthub/.agenthub`

The package also pre-creates `/var/lib/agenthub/workspaces` as a convenience
directory for repository checkouts, but AgentHub does not restrict workdirs to
it or any other path; operators can point agent/Team workdirs at any
filesystem path the `agenthub` service account can access.

Operators can edit that file and restart `agenthub.service`.

## Validation Matrix

- `sh -n` for Debian maintainer scripts.
- `bash -n build/deb/package.sh`.
- A package-script smoke test with dummy executable inputs to confirm `dpkg-deb` can build both
  `amd64` and `arm64` metadata.
- Release Prebuild push-to-main evidence that Linux targets upload both `.tar.gz` and `.deb`
  artifacts.
- Release-tag evidence that `SHA256SUMS.txt` includes `.deb` assets.

## Operational Notes

- The default service listens only on `127.0.0.1:8080`; production exposure should happen behind a
  reverse proxy or by editing the config deliberately.
- Since AgentHub does not restrict workdirs itself, the service user must be granted filesystem
  permissions for any repository roots operators point agent/Team workdirs at.
- Runtime data is not removed automatically. Operators should back up
  `/var/lib/agenthub/.agenthub` before upgrades.

## Open Risks

- Automatic service startup can fail when port `8080` is already in use; install still succeeds so
  operators can adjust config and restart.
- The package depends on distro runtime libraries such as `libsqlite3-0`; compatibility is validated
  through the Ubuntu release matrix first.
- Package signing and apt repository publication are deferred.

## Source Journals

- [2026-06-13 Debian systemd release package](../journal/2026-06-13-debian-systemd-release-package.md)
- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
