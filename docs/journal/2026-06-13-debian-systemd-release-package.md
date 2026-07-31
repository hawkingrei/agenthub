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
- `gh run list --workflow "Release Prebuild" --limit 10`
  - run `29639782865`
  - title `test(ci): add object-store s3 minio fixture (#890)`
  - branch `main`
  - event `push`
  - status `completed`
  - conclusion `success`
  - started `2026-07-18T09:48:34Z`
- `gh api repos/hawkingrei/agenthub/actions/runs/29639782865/artifacts`
  - `release-prebuild-x86_64-unknown-linux-gnu`
  - `release-prebuild-aarch64-unknown-linux-gnu`
  - `release-prebuild-aarch64-apple-darwin`
- `gh run view 29639782865 --log | rg "agenthub_.*\.deb|release-prebuild-|Uploading artifact|Artifact name"`
  - `dpkg-deb` built `dist/agenthub_0.0.0+main_amd64.deb`
  - `dpkg-deb` built `dist/agenthub_0.0.0+main_arm64.deb`
  - uploaded `release-prebuild-x86_64-unknown-linux-gnu.zip`
  - uploaded `release-prebuild-aarch64-unknown-linux-gnu.zip`
- `gh release view v0.0.11 --json tagName,name,publishedAt,url,assets`
  - release `v0.0.11`
  - published `2026-07-12T13:44:59Z`
  - includes `agenthub_0.0.11_amd64.deb`
  - includes `agenthub_0.0.11_arm64.deb`
  - includes `SHA256SUMS.txt`
- `gh release download v0.0.11 --pattern SHA256SUMS.txt --dir /private/tmp/agenthub-release-v0.0.11-check --clobber`
  - `SHA256SUMS.txt` includes `agenthub_0.0.11_amd64.deb`
  - `SHA256SUMS.txt` includes `agenthub_0.0.11_arm64.deb`

## Follow-Ups

- Decide separately whether to add signed apt repository publication.
