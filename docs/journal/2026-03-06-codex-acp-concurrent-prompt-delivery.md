# 2026-03-06 Codex ACP Concurrent Prompt Delivery

## Context

AgentHub persisted and rendered ACP user messages immediately, but the runtime wrapper still forced
all ACP commands except `Cancel` through a single in-flight prompt gate. In practice this meant a
new user message could appear in the conversation timeline while the Codex runtime did not receive
it until the previous turn completed.

`agenthub-codex-acp` already tracks prompt submissions by `submission_id`, so the blocking behavior
was local to AgentHub's outer ACP session loop rather than Codex ACP itself.

## Implementation

- added `AcpPromptDeliveryPolicy` to `crates/agenthub-acp` and threaded it through
  `SpawnAcpSessionRequest`.
- kept `gemini` and `kimi` on `StrictFifo`.
- enabled `AllowConcurrentPrompts` only for the `codex` provider.
- replaced the single `prompt_in_flight: bool` gate with `active_prompt_count: usize` so multiple
  Codex prompts can be in flight at once without releasing queued session-mutation commands early.
- kept `Cancel` as a passthrough command.
- kept `SetMode`, `SetModel`, and `SetConfig` serialized until all active prompts complete.

## Result

- Codex sessions can now accept a new user prompt while a previous turn is still working.
- Session-level config mutations remain ordered behind active prompts.
- Non-Codex ACP providers keep the previous conservative FIFO behavior.
- unexpected prompt-completion channel shutdown now aborts the ACP session loop instead of forcing
  `active_prompt_count` to zero and releasing queued session mutations unsafely.
- once a session mutation is queued behind active Codex prompts, later prompts now queue behind
  that mutation barrier instead of continuing to overtake it indefinitely.

## Follow-up

- `AllowConcurrentPrompts` still has no explicit max concurrency cap, so a separate backpressure
  limit may still be desirable if prompt fan-in becomes large enough to stress the runtime.

## Validation

- `cargo fmt --all`
- `cargo test -p agenthub-acp prompt_delivery_policy_is_provider_aware`
- `cargo test -p agenthub --lib --no-run`
