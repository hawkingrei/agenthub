# Actor Mailbox Hint Nudges

## Summary

Align CLI-first Team actor sends with the existing human/team API mailbox-hint
behavior so direct `agent -> agent` mailbox sends can proactively nudge the
target ACP session instead of waiting for manual inbox polling.

## Scope

- `src/team/mailbox_hint.rs`
- `src/team/mod.rs`
- `src/api/teams.rs`
- `src/actor_cli.rs`
- `docs/todo.md`

## Key Decisions

1. Move mailbox-type hint planning into a shared Team-domain helper.
   - `teams.rs` and `actor_cli.rs` now reuse the same payload-type extraction,
     duplicate-suppression, and prompt generation logic.
2. Keep CLI-first runtime semantics.
   - `actor send` still writes via Team mailbox service first.
   - Hint nudges are follow-up best-effort side effects; they must not fail the
     send itself.
3. Prefer an existing remote mailbox client when actor runtime env already
   points at an authority node; otherwise, if internal gRPC is enabled locally,
   build a local internal control client and send the hint through
   `SendAgentInput`.
4. If no agent-input channel exists, skip the nudge quietly.
   - This preserves single-binary/local-dev behavior even when internal gRPC is
     disabled.

## Validation

```bash
cargo test -p agenthub actor_cli::tests::actor_send_type_hint_is_best_effort_without_internal_grpc_client -- --nocapture
cargo test -p agenthub mailbox_type_hint_helpers_build_prompt_contains_context -- --nocapture
cargo clippy --locked -p agenthub --all-targets -- -D warnings
cargo fmt --all
git -c core.fsmonitor=false diff --check
```

## Follow-up

- Validate on a deployed Team session that a direct `actor send --to-actor-id`
  from one agent causes the destination ACP session to receive the same style of
  mailbox-type nudge that human/team API sends already trigger when an agent
  inbox item arrives.
