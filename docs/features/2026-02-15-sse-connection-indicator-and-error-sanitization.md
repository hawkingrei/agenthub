# SSE Connection Indicator And Error Sanitization

## Background

When AgentHub runs behind Cloudflare, stream interruptions can surface gateway HTML
responses in the UI error banner. This is noisy and confusing for users.
At the same time, the header did not expose whether the client is online or whether
the SSE stream is currently connected.

## Scope

- Add a header connection badge near the authenticated username.
- Surface network + SSE state transitions in a compact user-facing label.
- Sanitize error-banner messages so HTML/gateway responses are mapped to stable
  connectivity messages instead of rendering raw HTML payload text.

## Key Decisions

- Introduce a dedicated frontend helper module:
  - `deriveConnectionBadge(networkOnline, hasStreamTarget, sseState)`
  - `sanitizeErrorBannerMessage(rawMessage, networkOnline)`
- Keep SSE lifecycle states explicit in app state:
  - `idle`, `connecting`, `connected`, `reconnecting`.
- Drive network awareness from browser `online`/`offline` events plus runtime
  checks on SSE error callbacks.
- Keep sanitization at the error banner source layer (in `App`) so all error paths
  benefit from the same normalization.

## Validation

```bash
cd web
npm run test -- src/connection_status.test.ts src/agent_ws.test.ts
npm run build
```

- Expect:
  - connection badge labels/tone map correctly for offline/idle/connected/reconnecting;
  - HTML-like gateway payloads are normalized to a compact reconnecting message;
  - offline mode always shows a stable cannot-connect message.

## Follow-up

- Optionally expose a tooltip with last successful SSE open timestamp and retry
  backoff seconds for deeper operational troubleshooting.
