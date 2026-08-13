---
sidebar_position: 4
---

# API Error Reference

AgentHub HTTP errors use a small JSON envelope:

```json
{
  "error": "human-readable message"
}
```

Use the HTTP status for control flow. Error text is useful for diagnosis but is
not a stable machine-readable code.

## Status Codes

| Status | Meaning | Client action |
|--------|---------|---------------|
| `400 Bad Request` | The payload, parameter, state transition, or path is invalid. | Correct the request; do not retry unchanged. |
| `401 Unauthorized` | Bearer token is missing/expired, or the account lacks the required capability. | Sign in again or use an appropriately authorized account. |
| `403 Forbidden` | A route-specific policy rejected an authenticated action. | Change authorization or request scope. |
| `404 Not Found` | The resource is absent or not visible in the requested scope. | Refresh IDs and parent scope. |
| `409 Conflict` | Current state or a concurrent update conflicts with the request. | Reload state, then retry only if still valid. |
| `500 Internal Server Error` | An unexpected database, runtime, provider, or internal transport failure occurred. | Preserve logs and retry only after checking whether the operation committed. |

Capability failures are currently returned as `401` messages such as
`runtime:inspect required` or `root required`; do not assume every authorization
failure uses `403`.

## Common Authentication Errors

### `missing authorization token`

Add the normal bearer header:

```bash
curl --fail \
  -H "Authorization: Bearer ${AGENTHUB_TOKEN}" \
  http://127.0.0.1:8080/api/agents
```

### `invalid token`

The token is unknown, expired, or revoked. AgentHub browser/API sessions expire
after 12 hours. Sign in again; there is no refresh-token contract.

### `<capability> required` or `root required`

The session is valid but the role cannot perform the operation. See
[Security and Path Safety](./security-and-path-safety.md) for the role matrix.

## Common Validation Errors

- Agent creation rejects empty names/commands and workdirs outside the
  effective safe paths.
- Remote-node operations reject missing peer configuration, invalid node IDs,
  or unreachable internal gRPC targets.
- Agent loop enablement requires a prompt and an idle interval from 10 to
  86,400 seconds.
- History requests clamp `limit` to 1 through 20 and page older records with
  `before_id`.
- Team requests validate parent Team/run/task scope and state transitions.

Always compare a request with the exact `/api/openapi.json` document shipped by
the target binary when the operation is covered by OpenAPI.

## Diagnosing an Error

1. Record the method, path, HTTP status, and `error` value.
2. Confirm `/health` succeeds on the same origin.
3. Confirm the token is sent in an `Authorization` header, except for browser
   SSE URLs that use the documented query token.
4. Validate JSON and `Content-Type: application/json`.
5. Reload the resource before retrying a conflict.
6. Inspect the matching server log. Internal errors log their chained causes
   server-side while returning only the top-level message to the client.

When reporting a problem, include the AgentHub version, sanitized request body,
response status/body, and matching log window. Redact bearer/query tokens,
credentials, uploaded content, and private prompt data.
