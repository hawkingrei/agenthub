# Actor CLI Output Preference Contract

## Summary

- extracted a small `actor_output_preference_for_command(...)` helper in `src/actor_cli.rs`
- routed actor CLI commands through the shared output-preference helper
- kept read-heavy commands and human-oriented task/trigger confirmations on default TOON output
- kept `ack`, `send`, and `permission-review-respond` on default JSON output
- added focused regression tests to lock the conservative output split and `--json` override behavior

## Why

The previous CLI behavior mixed implicit defaults across branches in `run_actor_command(...)`.
Making the command-to-output mapping explicit keeps the contract readable and prevents future output
drift when CLI branches are edited. The chosen split is intentionally conservative:

- TOON for read-heavy output and human-oriented task/trigger confirmations
- JSON for machine-oriented acknowledgements and mailbox send confirmations

## Validation

- `cargo fmt --all`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
