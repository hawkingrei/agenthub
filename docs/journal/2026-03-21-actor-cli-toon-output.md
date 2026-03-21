## Summary

Changed `agenthub actor` structured stdout output from compact JSON to TOON via `toon-format`.

## Scope

- `agenthub actor team-members`
- `agenthub actor inbox`
- `agenthub actor ack`
- `agenthub actor send`

## Notes

- `help` output remains plain text.
- Error paths remain non-TOON command failures; callers should continue handling non-zero exit status separately from structured stdout parsing.
- `inbox` still prints only the `messages` array, not the full inbox wrapper with `next_cursor`.

## Validation

- Rebuild the root `agenthub` crate and run `actor_cli`-focused tests.
- Manually spot-check one success path per subcommand to confirm TOON stdout is emitted.
