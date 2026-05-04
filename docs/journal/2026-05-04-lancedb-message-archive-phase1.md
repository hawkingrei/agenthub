# LanceDB Message Archive Phase 1

## Summary

Started the first bounded slice for a LanceDB-backed message archive:

- documented the canonical archive contract
- defined the need for a backend-agnostic archive interface
- defined ACP chunk aggregation as a first-class archive concern instead of a UI-only concern

## Background

AgentHub currently persists message-like history across multiple SQLite surfaces:

- global and per-agent `agent_events`
- `team_conversation_messages`
- `team_run_events`
- `team_actor_messages`

The user asked to introduce LanceDB for message persistence and search, migrate existing SQLite
message history into LanceDB, and aggregate multi-event ACP messages before archive storage. An
additional requirement was to keep the storage abstraction friendly to multiple database backends,
not only LanceDB.

## Scope

Phase 1 is intentionally narrow:

- define the stable archive contract
- define backend abstraction boundaries
- define ACP aggregation behavior
- prepare the first Rust scaffold for the new archive layer

This phase does not yet flip the global message source of truth or remove legacy SQLite tables.

## Key Decisions

- Treat message archive as a separate retrieval/search plane instead of collapsing all relational
  Team runtime state into LanceDB immediately.
- Make backend abstraction mandatory from the first slice so LanceDB does not leak through
  business-facing interfaces.
- Aggregate ACP chunk events into logical messages before indexing them into the archive.
- Keep the first search contract text-first, using full-text search over canonical `body_text`.

## Validation

Executed for this rollout:

```bash
cargo test -p agenthub-config
cargo check -p agenthub-message-archive --tests
cargo check
cargo fmt --all --check
```

Observed during local validation:

- `cargo test -p agenthub-message-archive` reached the final linker stage but failed with
  `ld: write() failed, errno=28 (No space left on device)`.
- Until disk pressure is cleared, the practical local proof for the new crate is
  `cargo check -p agenthub-message-archive --tests` plus PR CI.

## Follow-Ups

- Wire the first backend-agnostic archive crate into the workspace.
- Add LanceDB bootstrap and full-text search tests.
- Add live dual-write hooks for new message-shaped records.
- Add idempotent historical migration from SQLite message tables and per-agent event databases.
