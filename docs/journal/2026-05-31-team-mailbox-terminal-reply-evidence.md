# Team Mailbox Terminal Reply Evidence

## Summary

Open reply-obligation summaries now require explicit terminal evidence before
reply-required human work disappears from the unresolved count.

## Background

The runtime already rejects new reply-required completions without user-visible
reply evidence and records explicit ignore reasons for valid ignored triage.
Summary and invariant traversal still needed to resist legacy or malformed rows
whose disposition looked terminal without carrying the evidence required by the
Team mailbox contract.

## Scope

- Treat `ignored` as terminal only when `mailbox_resolution.kind = "ignored"`
  and `mailbox_resolution.reason` is non-empty.
- Stop treating bare `completed` disposition as terminal evidence in reply
  obligation traversal; visible reply credit remains the way completed work
  clears the obligation.
- Preserve the existing `released` terminal exceptions for explicit
  `escalated`, `transferred`, and `taken_over` mailbox resolutions.
- Project `mailbox_resolution.reason` through both in-memory and SQL snapshot
  loaders so the lightweight summary path enforces the same rule.
- Reject completion guard checks when the target reply-required message is not
  explicitly matched to visible reply credit during traversal.

## Key Decisions

- Completion evidence stays tied to visible outbound reply matching rather than
  a disposition flag. This keeps summaries aligned with the public contract and
  avoids silently hiding unresolved user work.
- Ignore evidence is checked at the mailbox resolution layer instead of trusting
  disposition alone. This keeps malformed historical rows actionable until an
  operator records an allowed reason.
- This slice does not close the whole phase 3 audit. It narrows the remaining
  work to any future terminal outcomes that can end human-visible work without
  explicit evidence.
- Completion guard checks now fail closed when the target message is skipped as
  already terminal or is otherwise absent from the reply-obligation traversal,
  unless the target terminal message has an explicit visible reply credit. A
  terminal ignored/escalated/transferred/taken-over source cannot later be
  converted into `completed` without visible reply evidence.

## Validation

```bash
cargo test -p agenthub summarize_open_reply_obligations -- --nocapture
cargo test -p agenthub team_run_messages_api_triage_ignored_clears_open_reply_obligation_without_visible_reply -- --nocapture
cargo test -p agenthub team_run_messages_api_triage_resolves_open_reply_obligation -- --nocapture
cargo test -p agenthub actor_mailbox_service_rejects_complete_after_terminal_ignore_without_visible_reply -- --nocapture
cargo test -p agenthub actor_mailbox_service_allows_complete_after_terminal_ignore_with_visible_reply -- --nocapture
cargo test -p agenthub actor_mailbox_service_completed_claim_remains_visible_in_history -- --nocapture
cargo fmt --all --check
git -c core.fsmonitor=false diff --check
cargo clippy -p agenthub --all-targets -- -D warnings
```

## Follow-Ups

- Phase 3 closeout is tracked in
  [2026-06-02-team-mailbox-phase3-closeout.md](2026-06-02-team-mailbox-phase3-closeout.md).
