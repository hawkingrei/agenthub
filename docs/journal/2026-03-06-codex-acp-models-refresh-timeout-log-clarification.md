# 2026-03-06 Codex ACP Models Refresh Timeout Log Clarification

## Context

`agenthub-codex-acp` still uses codex git revision
`c34b30a3c128bb75fcec27ef838c93c99b92fc61`.

In that upstream revision, `codex_core::models_manager::manager` wraps the remote
`/models` request in a 5 second timeout, but maps timeout failures to the generic
`CodexErr::Timeout` variant. The shared error message for that variant is:

- `timeout waiting for child process to exit`

That wording is correct for some command execution paths, but misleading for model
refresh logs because the failure there is an HTTP timeout, not a child process
shutdown problem.

## Decision

Keep behavior unchanged and only fix the local log wording inside
`agenthub-codex-acp`.

Instead of patching the upstream dependency immediately, install a local tracing
formatter that rewrites only this one known misleading log line:

- target: `codex_core::models_manager::manager`
- level: `ERROR`
- original message:
  `failed to refresh available models: timeout waiting for child process to exit`

The rewritten message is:

- `failed to refresh available models: timed out fetching remote model list (/models, 5s timeout)`

## Scope

- `agenthub-codex-acp/src/lib.rs`

## Notes

- This change is intentionally presentation-only.
- Model refresh behavior, retry policy, timeout value, and fallback behavior remain
  unchanged.
- The earlier implicit websocket disable shim remains separate and unaffected.

## Validation

- `cargo +1.93.1 test -p agenthub-codex-acp --lib`
- result: `34 passed; 0 failed`
