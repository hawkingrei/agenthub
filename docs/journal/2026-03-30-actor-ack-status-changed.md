# Actor Ack Status Changed

## Summary

Tighten the `agenthub actor ack` response contract so callers can distinguish a
real mailbox state transition from an idempotent/no-op ack.

## Changes

- extend the actor mailbox ack contract with `status_changed`
- plumb the field through internal gRPC proto, server, client, and CLI output
- document the new `actor ack` diagnostic semantics
- add focused regression coverage for duplicate ack behavior

## Validation

- inspect `agenthub actor help ack` and confirm it explains `status_changed`
- confirm `AckActorMessageResponse` now carries `status_changed`
- confirm duplicate ack coverage exists in Team mailbox tests

## Verified Evidence

- Focused regression coverage stayed on duplicate ack semantics and mailbox response plumbing.
- `pull_request` CI for PR `#258`:
  - Bazel: `23739129183`
  - Rust: `23739129203`
  - Clippy: `23739129205`
  - Web: `23739129209`
  - Web E2E: `23739129181`
  - User Docs: `23739129230`
  - Distributed P2P Pipeline: `23739129240`
- default-branch `push` CI after merge commit `ac62fa37`:
  - Bazel: `23740075994`
  - Rust: `23740076042`
  - Clippy: `23740076055`
  - Web: `23740075987`
  - Web E2E: `23740075999`
  - User Docs: `23740076003`
  - Distributed P2P Pipeline: `23740075995`
