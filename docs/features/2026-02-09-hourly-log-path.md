---
title: Hourly Log Files via log_path
date: 2026-02-09
status: implemented
---

## Summary

Add `log_path` configuration to write all logs to hourly-rotated files.

## Background

When running locally or on a server, it is useful to persist logs without
relying on stdout. This change allows operators to configure a directory
for log files while keeping the default stdout behavior for simple setups.

## Decision

- Introduce a top-level `log_path` configuration key.
- If set, log output goes to `log_path/agenthub.log.YYYY-MM-DD-HH`.
- If unset, logs continue to go to stdout.
- Logs are written without ANSI color codes.

## Scope

- `Cargo.toml`
- `src/config.rs`
- `src/main.rs`

## Validation

- Configure `log_path` and confirm log files are created hourly.
- Start the server and verify stdout is quiet when `log_path` is set.
