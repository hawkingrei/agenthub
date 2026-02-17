# Codex ACP Upstream Sync: Path Resolution and JSON Ordering

## Summary

Sync two recent upstream `zed-industries/codex-acp` fixes into
`agenthub-codex-acp`:

1. Resolve relative filesystem paths against the ACP session root.
2. Enable `serde_json/preserve_order` to keep object key ordering stable.

## Upstream References

- `6159785cfa0a68f47d21699b28adfbbff9122e33`
  - `Support relative path in apply_patch`
- `23d88414d9bc7e189a2e9ef993b5c45c7b1ac638`
  - `Enable preserve_order for serde_json`

## Background

`agenthub-codex-acp` inherits behavior from `codex-acp`, but two recent fixes
were missing locally:

- Relative paths in `apply_patch` were resolved from process CWD, not the ACP
  session root, which can reject valid edits or allow confusing path behavior.
- JSON object ordering could diverge from codex-cli defaults, which affects
  hash-sensitive keychain or MCP payload processing.

## Scope

- `agenthub-codex-acp/src/local_spawner.rs`
- `agenthub-codex-acp/Cargo.toml`
- `docs/todo.md`

## Key Decisions

1. Keep path enforcement rooted at registered session root and explicitly
   resolve relative paths from that root.
2. Add lexical path normalization (`.` / `..`) before root-prefix checks to
   reject traversal attempts deterministically.
3. Enable `serde_json` `preserve_order` feature in `agenthub-codex-acp` to
   align JSON behavior with upstream codex-acp/codex-cli.
4. Add focused unit tests around `local_spawner` path guard behavior.

## Validation

```bash
cargo test -p agenthub-codex-acp local_spawner -- --nocapture
```

Expected outcomes:

- Relative patch paths resolve under session root.
- Paths escaping session root are rejected.
- Absolute in-root paths continue to work.
- Local ACP adapter tests pass with preserved JSON ordering enabled.
