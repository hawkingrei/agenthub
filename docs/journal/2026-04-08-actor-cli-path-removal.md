# Actor CLI Path Removal

## Summary

- removed `actor_cli_path` from the actor runtime request/context contract
- stopped exporting `AGENTHUB_ACTOR_CLI` into spawned actor processes
- updated managed runtime skills to use the stable `agenthub actor ...` form directly
- kept Codex runtime actor-command auto-approval by resolving the current executable instead of reading an injected actor-cli env var

## Validation

- `cargo test -p agenthub-acp -p agenthub-managed-skills -p agenthub-codex-acp --no-run`
- `cargo test -p agenthub --no-run`
