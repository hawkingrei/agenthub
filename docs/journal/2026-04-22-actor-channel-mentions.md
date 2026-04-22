# Actor Channel Mention Hardening

## Context

Team channel mention routing already preserved `@member_id` metadata on the backend, but
`agenthub actor send` still depended on raw text scanning to express channel mentions. That was
fragile for scripted sends and generated payloads because a sender had to embed literal
`@member_id` tokens in markdown to get stable mention metadata.

## Changes

- added explicit `agenthub actor send --mention <member_id>` / `--mention-actor-id <member_id>`
  flags for channel sends;
- normalized explicit mentions into `payload.mention_actor_ids` at CLI parse time so idempotency
  keys include the final routed mention metadata;
- rejected explicit mention flags on direct mailbox sends to keep the contract channel-scoped;
- taught backend mention extraction to also honor `mentioned_actor_ids` as an input alias when
  normalizing channel payloads and API task messages.

## Validation

- targeted parser regression:
  - `cargo test parse_send_channel_mentions_into_payload_and_dedupes`
  - `cargo test parse_send_rejects_direct_mentions`
- targeted mailbox regression:
  - `cargo test actor_mailbox_service_channel_send_honors_explicit_mentions_without_raw_text`
