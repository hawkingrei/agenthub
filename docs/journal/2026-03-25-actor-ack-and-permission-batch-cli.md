# Actor Ack And Permission Review Batch CLI

## Background

Team sessions often need to acknowledge several consumed mailbox messages in
one step, and reviewers often need to respond to several permission requests in
one shell command.

## Scope

- `agenthub actor ack` now accepts repeated `--message-id` flags.
- `agenthub actor permission-review-respond` now accepts repeated
  `--permission-id` flags.
- The CLI keeps the existing single-request mailbox/internal gRPC protocols and
  performs batch handling sequentially on the client side.

## Key Decisions

- Single-item output remains unchanged for backward compatibility.
- Multi-item runs return a JSON array of per-request responses.
- Persistent or session-scoped approval paths continue to use the
  request-provided `--option-id`; `--outcome` still supports only `cancelled`.

## Follow-ups

- Keep userdocs aligned with future `agenthub actor ...` CLI surface changes so
  operator-facing behavior does not live only in journals.

## Validation

- `cargo test parse_ack_ -- --nocapture`
- `cargo test parse_permission_review_respond_ -- --nocapture`
- `cargo test ack_actor_messages_batches_requests_in_order -- --nocapture`
- `cargo test -p agenthub-managed-skills managed_skill_docs_include_expected_frontmatter -- --nocapture`
- `cargo clippy --bin agenthub --tests -- -D warnings`
- `git diff --check`
