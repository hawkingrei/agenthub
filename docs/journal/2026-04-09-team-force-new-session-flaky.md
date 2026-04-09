# Team Force New Session Flaky Test

## Summary

- stabilized `force_new_session_restarts_member_runtime_with_new_session_id`
- added a dedicated long-lived Team member test fixture for lifecycle-sensitive assertions
- kept the default seeded `agenthub actor` fixture unchanged for tests that still exercise actor CLI behavior

## Why

The flaky Bazel coverage failure on PR `#321` came from a test-only race, not from the `@chenglou/pretext` upgrade itself. The shared Team fixture seeds members with `agenthub actor` and no subcommand, which can exit before lifecycle assertions poll `running_session_id_for_agent(...)`.

The restart test only needs a deterministic long-lived process so it can verify `running -> forced_restart -> new_session_id`. It does not need full actor CLI behavior, so the test now explicitly swaps the affected member agents onto a stable `/bin/sh -c 'exec sleep 3600'` command before starting the Team runtime.

## Validation

- `cargo test force_new_session_restarts_member_runtime_with_new_session_id -- --nocapture`
