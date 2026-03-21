## Summary

Changed the larger `agenthub actor` read responses from compact JSON to TOON via `toon-format`, while keeping small confirmation responses as JSON by default and adding a `--json` override for all structured success output.

## Scope

- `agenthub actor team-members`
- `agenthub actor inbox`
- `agenthub actor --json ...`

## Compatibility

- `agenthub actor ack` remains compact JSON.
- `agenthub actor send` remains compact JSON.
- `--json` forces JSON for `team-members` and `inbox` when shell or `jq` compatibility matters.

## Notes

- `help` output remains plain text.
- Error paths remain non-TOON command failures; callers should continue handling non-zero exit status separately from structured stdout parsing.
- `inbox` now prints the full inbox wrapper, including `next_cursor`, so pagination metadata is preserved.

## Validation

- Rebuild the root `agenthub` crate and run `actor_cli`-focused tests.
- Manually spot-check default `team-members` / `inbox` TOON stdout.
- Manually spot-check default `ack` / `send` JSON stdout.
- Manually spot-check `agenthub actor --json ...` for read commands that need JSON-compatible scripting.
