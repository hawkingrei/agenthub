# Default Gzip Compression

## Background

Static assets and JSON payloads benefit from gzip compression to reduce bandwidth usage and improve load times.

## Scope

- Enable `CompressionLayer` with gzip in the Axum stack.
- Keep the default compression predicate to avoid SSE and other non-compressible responses.

## Key Decisions

- Use `tower-http` gzip compression with defaults.
- Rely on `DefaultPredicate` to skip `text/event-stream` responses.

## Validation

- Manual: request a static asset with `Accept-Encoding: gzip` and verify `Content-Encoding: gzip`.
- Automated:

```bash
cargo test -p agenthub -- tests/web_assets.rs
```

## Follow-ups

- Consider adding brotli once the deployment path is confirmed.
