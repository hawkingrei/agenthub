# Daemon Instance Ownership

## Problem

Two daemon processes configured with the same logical node identity and SQLite database can both run
startup recovery, serve requests, and publish shutdown state. SQLite serializes individual
transactions, but it does not establish which process owns the runtime. A replacement process also
needs a durable generation token so delayed work from an older owner can be rejected.

## Scope

- Define daemon ownership by canonical database path and effective node ID.
- Acquire ownership before database-backed startup mutations.
- Persist a monotonically increasing generation for each node ID.
- Fence lifecycle writes when the caller no longer owns the current generation.
- Keep lock metadata available for local diagnostics.

## Non-Goals

- Distributed leader election across hosts that do not share a lock-capable filesystem.
- Replacing SQLite transaction isolation or entity-specific generation guards.
- Allowing two processes to serve the same node identity for availability.
- Deleting lock files during normal shutdown.

## Architecture

`agenthubd` canonicalizes the configured database path and acquires an operating-system advisory file
lock before opening the application database. The lock file is stored beside the database and its name
is a SHA-256 digest of the canonical database path and effective node ID. Different node IDs may share
one database, while the same node ID may independently own different databases.

After the database schema is ready, but before root-user creation, startup recovery, or background
workers, the daemon atomically claims the next row in `daemon_generations`. The in-process guard owns
both the locked file handle and the returned generation for the full server lifetime.

The lock file contains diagnostic JSON with the canonical database path, node ID, owner UUID, process
ID, start time, and claimed generation. The file is never removed on release: closing the handle
releases the advisory lock, while retaining the inode prevents lock-file replacement races.

## Contracts

### Lock Scope

- At most one live process owns a `(canonical database path, effective node ID)` pair.
- Equivalent paths, including a symlinked parent on Unix, resolve to the same lock scope.
- A conflicting process fails startup before database initialization or recovery writes.
- Dropping the guard releases ownership through operating-system file-handle semantics.

### Generation Claim

- Each successful claim for a node ID increments its durable positive integer generation.
- Node IDs maintain independent generation sequences in the shared database.
- A generation token is current only when both its generation number and owner UUID match the
  authoritative row.
- Schema creation and generation claim complete before any daemon-owned startup mutation.

### Fenced Lifecycle Writes

- Server startup verifies that its claimed generation remains current before accepting the runtime
  role.
- Shutdown cleanup verifies the generation again before publishing terminal agent state.
- A stale owner skips shutdown cleanup rather than overwriting state published by its replacement.
- Normal coexistence prevention is provided by the held advisory lock; generation verification is the
  durable defense for delayed lifecycle work and future fenced operations.

### Diagnostics

- Lock metadata contains ownership identifiers only and must not contain credentials or configuration
  secrets.
- Metadata is truncated and rewritten only after the process has acquired the advisory lock.
- Lock files may remain after a crash; their existence alone does not indicate a live owner.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Same identity | A second guard for the same database and node fails while the first is held. |
| Scope separation | Different nodes on one database and one node on different databases both succeed. |
| Canonical path | Real and symlinked parent paths collide on Unix. |
| Release | Dropping the guard permits a replacement guard to acquire the same identity. |
| Generation | Claims start at one, increment per node, and reject an older owner token. |
| Ordering | Daemon initialization claims ownership before root creation and startup recovery. |
| Shutdown fence | Cleanup checks the generation before publishing terminal runtime state. |
| Quality gates | Focused tests, `cargo fmt`, `cargo check`, and `cargo clippy -D warnings` pass. |

## Operational Notes

- The database directory must support the platform's advisory file-lock implementation.
- Operators diagnosing a startup conflict may inspect the JSON lock file, then confirm the recorded PID
  independently; they must not delete the file to force ownership.
- A crash releases the operating-system lock automatically. The next owner reuses the same file and
  advances the database generation.
- Main and node roles use their effective configured node IDs, so one shared database can support
  distinct node processes without sharing an ownership identity.

## Open Risks

- Filesystems with missing or unreliable advisory-lock semantics are unsupported for co-located daemon
  ownership and require an external singleton supervisor.
- Generation checks currently fence daemon lifecycle boundaries. Entity-specific asynchronous work
  should carry its own existing lease/generation or adopt this daemon token when cross-generation
  publication becomes possible.
- Process supervision, global spawn scheduling, durable delivery receipts, and unified task shutdown
  remain separate runtime-hardening slices.

## Source Journals

- [Daemon instance ownership fencing](../journal/2026-08-28-daemon-instance-ownership.md)
- [Two-binary runtime consolidation](../journal/2026-08-27-two-binary-runtime-consolidation.md)
