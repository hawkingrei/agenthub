# SSE Streaming For Agent Output

## Background

WebSocket was used for bidirectional streaming, but the frontend only requires server-to-client output. SSE is more proxy-friendly and simpler to operate.

## Scope

- Replace WebSocket output streaming with SSE (`/sse/agents/:id`).
- Keep input submission over HTTP (`/api/agents/:id/input`).
- Add SSE heartbeat events to prevent idle disconnects.

## Key Decisions

- Use query parameter tokens for SSE (`?token=`) to match existing WS behavior.
- Send heartbeat events as `data: heartbeat` and ignore them on the client.
- Keep polling as a fallback when SSE is not open.

## Validation

```bash
cargo test -p agenthub -- tests/web_assets.rs
cd web && npm test
```

## Follow-ups

- Consider removing the unused WS module and dependency feature once SSE is stable.
