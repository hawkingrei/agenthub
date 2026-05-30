# Team Mailbox Reply Ignore Reason Slice

## Summary

Reply-required human mailbox work can no longer be ignored without an explicit reason. The reason
is persisted on the source mailbox payload as `mailbox_resolution.kind = "ignored"` so operators can
distinguish an intentional no-reply outcome from a silent terminal state.

## Background

The mailbox phase 3 contract already required reply-required work to end through a user-visible
reply, reassignment, escalation, takeover, or an ignored outcome with an explicit allowed reason.
The previous completion guard enforced visible-reply evidence for `completed`, but `ignored` still
closed the open reply obligation without recording why.

## Scope

- Added `reason` to Team actor triage requests and commands.
- Required a non-empty reason when `ignored` is applied to human-originated reply-required work.
- Stored the ignore reason under `mailbox_resolution`.
- Kept non-reply-required ignore behavior unchanged.
- Passed the field through HTTP, CLI, and internal gRPC paths.

## Key Decisions

- The reason is optional at the transport contract level, but mandatory only for reply-required
  human work with `ignored` disposition.
- The reason is stored on the mailbox payload instead of adding a new table column, matching the
  existing escalation, transfer, and takeover resolution metadata model.

## Validation

Focused checks:

```bash
cargo fmt --all --check
git -c core.fsmonitor=false diff --check
cargo test -p agenthub team_run_messages_api_triage_ignored_clears_open_reply_obligation_without_visible_reply -- --nocapture
cargo test -p agenthub parse_triage_accepts_disposition_and_message_ids -- --nocapture
cargo test -p agenthub internal_grpc_mailbox_triage_and_task_link_are_wire_compatible -- --nocapture
```

## Follow-Ups

- Continue normalizing future human, trigger, and webhook intake into the canonical inbound
  envelope.
- Continue extending reply-required invariant coverage across future terminal outcomes as those
  outcomes are added.
