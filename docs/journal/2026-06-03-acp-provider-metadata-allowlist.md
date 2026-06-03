# ACP Provider Metadata Allowlist

## Summary

`agenthub doctor agent-trace` now exposes provider-native correlation metadata only through an
explicit allowlist. The diagnostic path keeps IDs and runtime counters that help debug stuck ACP
turns, while avoiding prompt text, message bodies, tool arguments, tool outputs, and command error
messages.

## Implementation

- Added `AgentTraceEvent.provider_metadata` for allowlisted provider-native event identifiers:
  session/thread/turn/item/request/submission/tool-call/permission ids and related trace ids.
- Expanded event redaction markers for body-like fields such as `message`, `input`, `output`,
  `arguments`, and tool-result payloads.
- Replaced live ACP provider `details` serialization with a hand-built safe diagnostic object:
  counts, channel state, submission ids, provider event class/timestamp, pending tool-call ids,
  stale-prompt counters, and command error kind only.
- Updated the ACP runtime spec to make provider-native diagnostics an explicit allowlist contract.

## Validation

Focused tests cover the stable boundary:

- `agent_trace_event_metadata_allowlist_keeps_ids_and_redacts_bodies`
- `provider_diagnostics_details_redact_error_messages_and_keep_safe_ids`

Recommended command set for this slice:

```bash
cargo test -p agenthub-diagnostics agent_trace_event_metadata_allowlist_keeps_ids_and_redacts_bodies
cargo test -p agenthub provider_diagnostics_details_redact_error_messages_and_keep_safe_ids
cargo fmt --all --check
cargo clippy -p agenthub --all-targets -- -D warnings
```

## Follow-Up

No active TODO remains for the provider-native metadata allowlist. Future provider metadata fields
must be added by extending the allowlist and proving they cannot carry prompt, message, tool
argument, or tool output bodies.
