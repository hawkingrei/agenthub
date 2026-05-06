# ACP SDK Unification

## Summary

Merged the workspace onto a single `agent-client-protocol` dependency at `0.11.1` and removed
the `agent-client-protocol-legacy` alias pinned to `0.10.4`.

## Background

The workspace had both ACP SDK generations in the Cargo graph. Runtime code still used the old
root-level schema exports and trait-based `AgentSideConnection` / `ClientSideConnection` APIs,
while the current SDK exposes schema types through `agent_client_protocol::schema` and uses
role builders with `ConnectionTo`.

## Scope

- Removed `agent-client-protocol-legacy` from workspace, ACP core, ACP runtime, and Codex ACP
  manifests.
- Updated ACP schema imports to use `agent_client_protocol::schema`.
- Migrated `agenthub-acp` client transport to `Client.builder().connect_with(...)`.
- Migrated `agenthub-codex-acp` agent transport to `Agent.builder().connect_to(...)`.
- Removed an unmounted Codex ACP local-spawner test harness that still referenced the deleted
  legacy connection API.
- Preserved the Codex ACP local runtime model with a narrowly scoped local future wrapper for the
  SDK's `Send` handler bounds.

## Key Decisions

- Keep the dependency version at `0.11.1`, which is the latest published `agent-client-protocol`
  crate version confirmed for this change.
- Keep business behavior in the existing ACP/Codex methods and isolate SDK migration code at the
  transport boundary.
- Use one `agent-client-protocol` package in `Cargo.lock`; the old `0.10.4` package and
  `agent-client-protocol-schema 0.11.4` are no longer present.

## Validation

```bash
cargo fmt --all --check
cargo check -p agenthub
cargo check -p agenthub-acp
cargo check -p agenthub-codex-acp
cargo test -p agenthub-acp -p agenthub-codex-acp --lib
```

## Follow-Ups

None.
