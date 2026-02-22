---
title: ck Search MCP Bootstrap and .info Research Layout
date: 2026-02-22
status: proposed
---

## Summary

Introduce a local bootstrap path so AgentHub ACP sessions can use `ck` (`BeaconBay/ck`) as an MCP search backend, and standardize local research storage layout for papers/clippings under `.info/`.

## Background

AgentHub already supports MCP/skills injection via `~/.agenthub/mcp.json` and `~/.agenthub/skills.json`.
For context-heavy research and implementation review, we need:

1. A deterministic way to register `ck --serve` as MCP server.
2. A stable local directory convention for research artifacts (`papers`, `clippings`) that remains git-ignored.

## Decision

1. Add bootstrap script:
   - `scripts/setup_ck_search_context.sh`
2. Script behavior:
   - update `~/.agenthub/mcp.json` (or custom path) with a named MCP server entry:
     - `command: ck`
     - `args: ["--serve"]`
     - `cwd: <configured path>`
   - create local research directories (unless disabled):
     - `.info/papers/`
     - `.info/clippings/`
3. Keep `.info/` non-versioned and treat it as local-only context.
4. Promote adopted decisions back into tracked docs (`docs/features/`, `docs/todo.md`).

## Scope

- `scripts/setup_ck_search_context.sh`
- `AGENTS.md`
- `docs/todo.md`
- `docs/features/2026-02-22-ck-search-mcp-and-info-research-layout.md`

## Usage

```bash
# default bootstrap
scripts/setup_ck_search_context.sh

# custom MCP config path and custom search root
scripts/setup_ck_search_context.sh \
  --mcp-file ~/.agenthub/mcp.json \
  --cwd /path/to/workspace

# skip .info layout creation if needed
scripts/setup_ck_search_context.sh --skip-info-layout
```

## Validation

- Local script validation:
  - `bash -n scripts/setup_ck_search_context.sh`
- Functional checks:
  - MCP config contains `ck-search` server with `ck --serve`.
  - `.info/papers` and `.info/clippings` exist after bootstrap (unless skipped).
  - ACP session can see ck MCP tools when runtime loads `~/.agenthub/mcp.json`.

## Risks

- `ck` binary missing on host:
  - script fails fast with explicit error.
- Incorrect `cwd` can scope search to wrong workspace:
  - script allows explicit `--cwd` override.

