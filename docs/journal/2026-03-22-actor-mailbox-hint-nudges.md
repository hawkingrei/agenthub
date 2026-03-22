# Actor Mailbox Hint Nudges

## Summary

Align mailbox nudges across CLI-first Team actor sends and human/team API
delivery while making the policy token-efficient by default.

## Scope

- `src/team/mailbox_hint.rs`
- `src/team/mod.rs`
- `src/api/teams.rs`
- `src/actor_cli.rs`
- `src/actor_mcp.rs`
- `src/agent/manager.rs`
- `src/state.rs`
- `src/team/manager.rs`
- `src/team/manager/mailbox.rs`
- `src/internal/service.rs`
- `src/internal/client.rs`
- `proto/internal/v1/team.proto`
- `docs/features/actor-foundation.md`
- `docs/todo.md`

## Key Decisions

1. Keep mailbox hints token-first.
   - Immediate ACP nudges are now reserved for:
     - direct `agent -> agent` mailbox sends;
     - leader-authored channel sends that explicitly mention recipients.
   - All other mailbox traffic avoids immediate prompt injection.
2. Add delayed unread summaries instead of eager prompt spam.
   - A background worker checks pending unread counts.
   - If an actor still has unread mail and its ACP session has produced no
     non-user output for roughly 3 minutes, AgentHub sends one compact unread
     summary prompt.
   - If unread count is `0`, no reminder is sent.
3. Make unread state visible in mailbox reads.
   - `actor inbox` now returns `pending_count` so agents can see unread load
     without waiting for a separate reminder path.
   - Internal gRPC `ListActorInbox` now carries the same count for p2p/runtime
     parity.
4. Keep CLI-first runtime semantics.
   - `actor send` still writes via Team mailbox service first.
   - Hint nudges remain best-effort follow-up side effects; they must not fail
     the send itself.
5. Prefer an existing remote mailbox client when actor runtime env already
   points at an authority node; otherwise, if internal gRPC is enabled locally,
   build a local internal control client and send the hint through
   `SendAgentInput`.
6. If no agent-input channel exists, skip the nudge quietly.
   - This preserves single-binary/local-dev behavior even when internal gRPC is
     disabled.

## Validation

```bash
cargo test -p agenthub actor_cli::tests::actor_send_type_hint_is_best_effort_without_internal_grpc_client -- --nocapture
cargo test -p agenthub dispatches_worker_permission_to_leader_and_can_fallback_to_human_review -- --nocapture
cargo test -p agenthub actor_mailbox_service_returns_contract_responses -- --nocapture
cargo test -p agenthub mailbox_type_hint_helpers_build_prompt_contains_context -- --nocapture
cargo test -p agenthub internal::service::tests -- --nocapture
cargo test -p agenthub-team-actor actor_inbox_with_auto_ack_marks_pending_as_delivered -- --nocapture
cargo test -p agenthub 'team::mailbox_hint::tests::' -- --nocapture
cargo clippy --locked -p agenthub-team-actor --all-targets -- -D warnings
cargo clippy --locked -p agenthub --all-targets -- -D warnings
./scripts/check_team_proto_codegen.sh --check
cargo fmt --all --check
git -c core.fsmonitor=false diff --check
```

## Follow-up

- Validate on a deployed Team session that:
  - direct `actor send --to-actor-id ...` still produces an immediate ACP nudge;
  - leader `channel_id=all` sends only produce immediate ACP nudges for directly
    mentioned recipients;
  - non-direct/non-mentioned mailbox traffic waits for the 3-minute idle window
    before sending a compact unread summary;
  - `actor inbox` visibly reports `pending_count`.
