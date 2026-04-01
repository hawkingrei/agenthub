# Release Assets Workflow Split

## Summary

- updated the GitHub `Release` workflow to publish split release binaries instead of one
  per-platform bundle that mixed both executables together
- kept the three supported binary targets:
  - Linux x86_64
  - Linux arm64
  - macOS arm64
- added an explicit source archive to the release assets
- added `SHA256SUMS.txt` so published assets can be verified after download

## Why

The existing release workflow already built release-mode binaries for the required targets, but it
published them as one archive per platform containing both `agenthub` and `agenthub-codex-acp`.

For distribution and automation, the release surface is cleaner if:

- each binary is published as its own asset
- source is attached as an explicit release artifact instead of relying only on GitHub's implicit
  source downloads
- release consumers get one checksum file that covers every shipped artifact

## What Changed

- `.github/workflows/release.yml`
  - packages `agenthub` and `agenthub-codex-acp` into separate archives for each target
  - downloads build artifacts into one release staging directory
  - creates `agenthub-<version>-source.tar.gz` from the tagged commit
  - generates `SHA256SUMS.txt` for all `.tar.gz` release assets
  - updates the release body to describe the new asset layout
- `docs/todo.md`
  - adds a post-merge verification item for the new release asset layout

## Validation

- `ruby -e 'require "yaml"; YAML.load_file(".github/workflows/release.yml")'`
- `git -c core.fsmonitor=false diff --check`

## Follow-Up

- verify one real tag or `workflow_dispatch` run publishes exactly:
  - 3 `agenthub` archives
  - 3 `agenthub-codex-acp` archives
  - 1 source archive
  - 1 `SHA256SUMS.txt`
