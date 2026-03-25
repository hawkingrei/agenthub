# Actor Ack And Permission Review Batch CLI

## Summary

- `agenthub actor ack` now accepts repeated `--message-id` flags.
- `agenthub actor permission-review-respond` now accepts repeated `--permission-id` flags.
- The CLI keeps the existing single-request mailbox/internal gRPC protocols and performs batch handling sequentially on the client side.
- Single-item output remains unchanged; multi-item runs return a JSON array of per-request responses.

## Why

- Team sessions often need to acknowledge several consumed mailbox messages in one step.
- Team reviewers often need to approve or cancel several pending permission requests in one shell command.
- The previous single-id CLI forced repeated invocations even though the authority-side validation path was identical for each request.

## Validation

- `cargo test parse_ack_ -- --nocapture`
- `cargo test parse_permission_review_respond_ -- --nocapture`
- `cargo clippy --bin agenthub --tests -- -D warnings`
- `git diff --check`
