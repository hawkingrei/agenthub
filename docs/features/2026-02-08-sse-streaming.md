# SSE Streaming For Agent Output

## Background

WebSocket was used for bidirectional streaming, but the frontend only requires server-to-client output. SSE is more proxy-friendly and simpler to operate.

## Scope

- Replace WebSocket output streaming with SSE (`/sse/agents/:id`).
- Keep input submission over HTTP (`/api/agents/:id/input`).
- Add SSE heartbeat events to prevent idle disconnects.
- Add SSE response headers to avoid proxy buffering and caching.
- Keep event sequence IDs within JS-safe integer bounds.

## Key Decisions

- Use query parameter tokens for SSE (`?token=`) to match existing WS behavior.
- Encode agent IDs and tokens when building the SSE URL.
- Send heartbeat events as `data: heartbeat` and ignore them on the client.
- Keep polling as a fallback when SSE is not open.
- Generate `seq` values with a monotonic microsecond clock to preserve ordering while staying JS-safe.
- Set `Cache-Control: no-cache`, `Connection: keep-alive`, and `X-Accel-Buffering: no`.

## Validation

```bash
cargo test -p agenthub -- tests/web_assets.rs
cd web && npm test
```

## Follow-ups

- Consider removing the unused WS module and dependency feature once SSE is stable.
