# Debian Systemd Release Package

## Summary

- Added Linux `.deb` release packaging for `agenthub` and `agenthub-acp`.
- Added a default `agenthub.service` that runs as the `agenthub` system user.
- Wired Debian package generation into `release.yml` and `release-prebuild.yml`.

## Background

Tarball releases are enough for manual installs, but Debian/Ubuntu hosts need a managed service path
that can install, enable, and start AgentHub consistently.

## Scope

- Linux `amd64` and `arm64` Debian package assets.
- Package-owned systemd unit and maintainer scripts.
- Documentation for install, service status, config, and uninstall behavior.

## Key Decisions

- Keep the existing runtime config contract and set `HOME=/var/lib/agenthub` in systemd instead of
  introducing a new service-only config flag.
- Generate a minimal service config only when `/var/lib/agenthub/.agenthub/config.toml` does not
  already exist.
- Preserve `/var/lib/agenthub` on remove and purge so package operations do not delete runtime data.
- Let install complete even if automatic service startup fails, then point operators to
  `journalctl -u agenthub.service`.

## Validation

- `bash -n build/deb/package.sh`
- `sh -n build/deb/DEBIAN/postinst build/deb/DEBIAN/prerm build/deb/DEBIAN/postrm`
- `build/deb/package.sh --version v0.0.0-test --arch amd64 --target x86_64-unknown-linux-gnu --output-dir /tmp/agenthub-deb-smoke/dist`
- `dpkg-deb --info /tmp/agenthub-deb-smoke/dist/agenthub_0.0.0-test_amd64.deb`

### 2026-07-20 Release Prebuild Verification

- Observed main `Release Prebuild` run `29748545683` after merge commit
  `676230bf6d664eb56559d5ed96fa3fa4ca44a136`.
- Confirmed the Linux prebuild package steps produced Debian packages:
  - `dist/agenthub_0.0.0+main_amd64.deb`
  - `dist/agenthub_0.0.0+main_arm64.deb`
- Confirmed both Linux artifact upload steps included `dist/*.deb` in the release-prebuild matrix
  bundles.

## Follow-Ups

- Verify the next release tag includes `.deb` files in `SHA256SUMS.txt`.
- Decide separately whether to add signed apt repository publication.
