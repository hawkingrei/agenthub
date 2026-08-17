# Summary

A code-only review of the Team subsystem found that any caller of the mailbox send API could set
`payload.requires_user_visible_reply: false` on a message from a human to an agent, silently disabling
the system's guarantee that a human message to an agent is tracked until visibly answered.
`normalize_actor_message_envelope_payload` only *backfilled* this field when absent from the caller's
payload; an explicit `false` was always honored verbatim. `send_team_run_message`
(`src/api/teams.rs`) passes the raw client-supplied JSON body straight through to the mailbox layer with
no field allowlist, so this was reachable from the public HTTP API by any authenticated caller with
runtime-operate capability, human or agent.

# Scope

- `crates/agenthub-team-actor/src/message.rs`: `infer_requires_user_visible_reply` now checks whether
  the message is human-to-agent *first*, using `from_actor_id`/`to_actor_id` identity (the "must always
  be tracked" case), before ever consulting the payload's `requires_user_visible_reply` field. Only when
  that isn't the case does the caller-supplied value (if present) apply, matching prior behavior for
  every other combination.

# Key Decisions

- **The check derives "is this from a human" from `from_actor_id`, not from `source_kind`.**
  `source_kind` is itself payload-derived (`infer_actor_message_source_kind` also trusts an explicit
  `payload.source_kind` when present) and could otherwise be spoofed the same way in the same request --
  confirmed with a test that sets both `source_kind: "agent"` and `requires_user_visible_reply: false`
  on a message whose `from_actor_id` is genuinely human, and asserts the reply obligation still holds.
  `from_actor_id` is the one signal here a sender can't rewrite to claim a different identity than the
  one it's actually sending as.
- **Scoped the fix to human-to-agent only, not "payload can never set this field."** Every other
  direction (agent-to-agent, agent-to-human) keeps the existing explicit-override behavior unchanged --
  this is relied on by legitimate internal callers, notably `mailbox_service_escalation.rs`'s
  reassign/escalate/transfer flow, which re-derives the envelope from the *original* message's
  `from_actor_id` so a human-originated request correctly keeps requiring tracking across a reassignment
  hop, without that flow needing its own special-case logic.
- Considered stripping `requires_user_visible_reply`/`source_kind` from the client payload at the HTTP
  boundary (`send_team_run_message`) instead. Rejected: that field-stripping approach only protects the
  one entrypoint it's applied to, and there are multiple current internal callers of
  `normalize_actor_message_envelope_payload` (mailbox_facade, mailbox_store_delivery, the mailbox
  escalation flow, a broadcast-forwarding path in `api/teams.rs`) that would each need the same
  treatment, with any future caller silently missing it. Fixing the shared inference function once
  closes the gap for all current and future callers uniformly.

# Validation

- New `project_actor_message_envelope_rejects_client_suppressed_human_reply_obligation` and
  `project_actor_message_envelope_rejects_spoofed_source_kind_alongside_suppressed_reply_obligation`
  (`message.rs`) in `agenthub-team-actor`. Confirmed both fail with the original code (reverted the fix
  locally and reran).
- `cargo test -p agenthub-team-actor` -- 34 passed, including all pre-existing
  `requires_user_visible_reply` tests unchanged (none of them exercise a human-to-agent message with an
  explicit `false`, so none needed updating).
- `cargo test --lib team::` (`agenthub`) -- 211 passed, no regressions.
- `cargo test --lib` -- 767 passed; 3 pre-existing `state::tests::*` failures (unrelated
  `lance-namespace-impls` panic) confirmed present on `main` before this change.
- `cargo clippy --lib --tests -p agenthub` and `-p agenthub-team-actor`, plus `cargo fmt -- --check` for
  both, clean.

# Follow-Ups

- The other findings from the same 2026-08-17 Team-subsystem review round remain open, tracked in
  `docs/todo.md`'s Agent Team Correctness item.
