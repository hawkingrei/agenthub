# Team Mailbox Phase 3 Closeout

## Summary

Team mailbox phase 3 now has a closed terminal-outcome audit for
`requires_user_visible_reply` obligations. No mailbox handling disposition can
hide human-visible work without either a matched visible reply or explicit
mailbox resolution evidence.

## Background

Earlier phase 3 slices guarded completed work, reassignment, takeover, and
ignored outcomes. The remaining closeout was to verify the full handling
disposition surface and harden any path where terminal-looking state could make
reply-required human work disappear from summaries or completion guards.

## Scope

- Audited the handling disposition set: `untriaged`, `watching`, `claimed`,
  `completed`, `ignored`, and `released`.
- Kept non-terminal dispositions visible in open reply-obligation summaries.
- Kept `completed` dependent on reverse-matched user-visible reply credit.
- Kept `ignored` terminal only with `mailbox_resolution.kind = "ignored"` and a
  non-empty `mailbox_resolution.reason`.
- Kept `released` terminal only for explicit `escalated`, `transferred`, or
  `taken_over` mailbox resolutions.
- Hardened the snapshot terminal check so blank ignored reasons cannot count as
  terminal evidence even if a caller constructs a snapshot directly.

## Key Decisions

- No new terminal outcome type is needed for phase 3. The existing disposition
  enum already has only one ambiguous terminal-looking state, `completed`, and
  it remains tied to visible reply credit.
- Snapshot traversal should fail closed on malformed terminal evidence. Loader
  normalization already trims empty SQLite values to `None`; the terminal check
  now also enforces non-empty reason text defensively.
- Dead-letter delivery state is not reply evidence. It remains excluded from
  visible reply credit and does not create a separate terminal exception for
  inbound human obligations.

## Validation

```bash
cargo fmt --all --check
git -c core.fsmonitor=false diff --check
```

Local focused test attempt:

```bash
cargo test -p agenthub summarize_open_reply_obligations_requires_ignored_resolution_reason -- --nocapture
```

This command reached final linking but could not complete in the local worktree
because the linker failed with `errno=28` (`No space left on device`). CI should
be the authoritative Rust test and clippy validation for this closeout.

## Follow-Ups

- No phase 3 terminal-outcome audit follow-up remains open.
