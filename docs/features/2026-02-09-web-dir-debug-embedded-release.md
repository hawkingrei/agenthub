---
title: Debug Uses web_dir, Release Forces Embedded Assets
date: 2026-02-09
status: implemented
---

## Summary

Serve frontend assets from `web/dist` during debug builds to reflect local
changes without forcing a Rust rebuild. Release builds always serve embedded
assets to ensure a self-contained binary.

## Background

Developers often run `make run` and expect frontend changes to appear
immediately. When assets are embedded, the Rust binary may not rebuild if only
`web/dist` changes. This leads to confusing stale UI behavior. In production
we still want a single binary with embedded assets.

## Decision

- Debug builds resolve `web_dir` to `web/dist` by default (or the configured
  `web_dir` when provided).
- Release builds ignore `web_dir` and always use embedded assets.

## Scope

- `src/config.rs`
- `src/main.rs`

## Validation

- Debug: run `make run`, update a frontend file, rebuild `web/dist`, and refresh
  the browser to confirm updates appear without rebuilding Rust.
- Release: run a release build and confirm logs show embedded assets while the
  UI still loads.
