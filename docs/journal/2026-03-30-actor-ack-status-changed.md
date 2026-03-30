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

