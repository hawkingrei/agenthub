# Team Mailbox Inbound Envelope Normalization

## Summary

Team actor mailbox writes now normalize payloads into the canonical inbound-envelope shape at the
`team_actor_messages` persistence boundary. This makes the envelope projection durable for direct
mailbox sends, shared-channel fan-out, remote relay input, trigger events, and future webhook
writers instead of relying on each caller to remember the same payload enrichment.

## Background

Mailbox phase 3 already defined the stable inbound-envelope contract, and recent slices added
reply-required transfer, takeover, and ignored-reason outcomes. The remaining intake gap was that
normalization still happened mainly in higher-level send helpers, while the store accepted raw
payloads. Any new writer could therefore persist a message without the canonical `source_kind`,
`source_surface`, `reply_target`, or `requires_user_visible_reply` fields.

## Scope

- Canonicalized non-object mailbox payloads into object payloads before persistence.
- Preserved stringified JSON object payloads by parsing them before enrichment.
- Backfilled blank or null canonical envelope fields from the inferred projection.
- Persisted normalized payloads before idempotency comparison, visible-reply mirroring, inbox reads,
  relay scans, and archive fan-out observe the message.
- Added focused coverage for agent text, human conversation, stringified JSON, and trigger-event
  normalization.

## Key Decisions

- Store-level normalization is the authoritative boundary. Higher-level callers may still normalize
  for early local decisions, but bypassing those helpers no longer creates non-canonical rows.
- Plain text payloads become minimal `chat_message` payloads because that is the established
  human-readable mailbox shape.
- Non-string scalar or array payloads are wrapped under `payload` with a `type` derived from the
  message kind, keeping machine-readable content while still making envelope fields queryable.
- Explicit non-empty canonical fields are preserved; blank strings or null reply targets are
  repaired because they are not valid stable envelope values.

## Validation

Focused checks:

```bash
cargo fmt --all
cargo test -p agenthub-team-actor normalize_actor_message_envelope_payload -- --nocapture
cargo test -p agenthub actor_messages_persist_canonical_inbound_envelope_for_text_payload -- --nocapture
cargo test -p agenthub summarize_open_reply_obligations_prefers_lightweight_snapshot_loader -- --nocapture
cargo test -p agenthub actor_messages_detect_pending_payload_type_by_actor_inbox -- --nocapture
cargo test -p agenthub actor_messages_support_inbox_and_ack_flow -- --nocapture
```

## Follow-Ups

- Continue the remaining Team mailbox phase 3 invariant audit for terminal outcomes that can close
  human-visible work.
