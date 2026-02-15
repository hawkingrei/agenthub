# A2A Team Actor Runtime Start Verification and Relay Replay Guidance

## Summary

Close two pending Team hardening items:

1. verify `/api/agents/:id/start` actor runtime payload wiring reaches process
   environment and ACP skill contract;
2. add receiver-side remote relay replay-protection guidance with a reference
   validation flow.

## Background

Recent Team work moved actor runtime context to explicit start payloads and added
relay signing/message-id headers. Remaining risk was operational: we had parser
tests but no route-level proof that actor runtime payload values are injected
into real agent process env, and we still lacked a concrete receiver-side replay
policy document.

## Scope

- `src/api/agents.rs`
- `crates/agenthub-acp/src/actor_runtime_skill.rs`
- `docs/todo.md`
- `docs/features/2026-02-15-a2a-team-actor-runtime-start-and-relay-replay-guidance.md`

## Key Decisions

1. Add route-level tests for `/api/agents/:id/start` with `actor_runtime`
   payload:
   - start a real subprocess (`/bin/sh`) and assert exported env values
     (`AGENTHUB_ACTOR_RUN_ID`, `AGENTHUB_ACTOR_ID`,
     `AGENTHUB_ACTOR_CHANNEL`, `AGENTHUB_ACTOR_CLI`).
2. Add unit test for ACP built-in actor runtime skill builder:
   - lock skill name/path contract;
   - assert rendered instruction block includes actor context fields and
     actor CLI command guidance.
3. Add relay signing header unit test to lock receiver handshake headers:
   - `X-AgentHub-Signature`
   - `X-AgentHub-Timestamp`
   - `X-AgentHub-Message-Id`
4. Define receiver-side replay-protection reference policy for remote relay:
   - validate HMAC signature first;
   - enforce timestamp skew window (recommended default: `±300s`);
   - reject duplicated `message_id` for the same sender/route policy window;
   - when available, also dedupe by idempotency key tuple
     `(run_id, from_actor_id, idempotency_key)`.

## Receiver Reference Flow

```text
1) Read headers:
   - X-AgentHub-Signature
   - X-AgentHub-Timestamp
   - X-AgentHub-Message-Id
2) Parse JSON body and extract envelope fields.
3) Verify timestamp is within allowed skew window.
4) Rebuild canonical signature input:
   "<message_id>.<timestamp>.<raw_body_bytes>"
5) Recompute HMAC-SHA256 with shared secret and compare in constant time.
6) Check replay store:
   - reject if message_id already seen in window;
   - if idempotency_key exists in payload metadata, reject incompatible reuse.
7) Persist accepted message and replay key in one transaction.
```

### Suggested Replay Store Keys

- `relay_msg:<sender_or_route_scope>:<message_id>`
- `relay_idem:<run_id>:<from_actor_id>:<idempotency_key>`

Use TTL aligned with retry horizon (for example 24h) and keep a structured
rejection reason (`expired_timestamp`, `bad_signature`, `duplicate_message_id`,
`idempotency_conflict`) for observability.

## Validation

```bash
cargo test start_route_with_actor_runtime_payload_injects_actor_envs
cargo test actor_runtime_skill_includes_context_and_cli_contract -p agenthub-acp
cargo test apply_route_signing_sets_signature_timestamp_and_message_id_headers
```

## Follow-ups

- Validate the replay policy against a real remote receiver deployment
  (staging), including clock skew and duplicate retry behavior.
- Add receiver-side metrics dashboard by rejection reason and endpoint.
