# Team Mailbox Phase 3 Takeover Slice

## Summary

Added an explicit takeover path for actively claimed reply-required Team mailbox work. Takeover now
releases the original actor's item with a `taken_over` resolution, creates a new target-actor
mailbox item that preserves the reply obligation, and moves the topic claim owner to the target
actor.

## Background

Phase 3 already separated normal triage from reply-required escalation and general transfer. The
remaining ownership gap was cross-actor takeover: a second actor still could not silently claim an
active topic, but operators also lacked an explicit outcome that both preserved the human reply
obligation and reassigned topic ownership.

## Scope

- Added the run message takeover API for claimed reply-required mailbox items.
- Recorded takeover as `mailbox_resolution.kind = "taken_over"` on the source item.
- Recorded target-side takeover metadata under `mailbox_takeover`.
- Preserved the open reply obligation on the target actor.
- Kept ordinary parallel claim attempts as conflicts.

## Key Decisions

- Takeover is a separate endpoint instead of another triage disposition. Triage remains the actor's
  handling state, while takeover is an operator-level ownership reassignment outcome.
- Takeover requires a currently claimed topic. Unclaimed handoff remains the transfer flow.
- The target item is inserted as claimed so the thread owner and mailbox handling state agree
  immediately after takeover.

## Validation

Focused checks:

```bash
cargo fmt --all --check
cargo test -p agenthub team_run_messages_api_triage_surfaces_takeover_state -- --nocapture
cargo test -p agenthub actor_mailbox_service_claims_topics_and_prevents_parallel_takeover -- --nocapture
git -c core.fsmonitor=false diff --check
```

## Follow-Ups

- Continue normalizing future human, trigger, and webhook intake into the canonical inbound
  envelope.
- Continue extending the `requires_user_visible_reply` invariant beyond the currently guarded
  completion and reassignment paths.
