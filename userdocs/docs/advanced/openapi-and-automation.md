---
sidebar_position: 2
---

# OpenAPI and Automation

AgentHub publishes a generated OpenAPI document for the HTTP operations that
currently have a stable automation contract. The document is intentionally
incremental; it does not yet describe every route used by the web UI.

## Discovery Endpoints

| Endpoint | Authentication | Purpose |
|----------|----------------|---------|
| `GET /api/openapi/docs` | None | Interactive API reference. |
| `GET /api/openapi.json` | Bearer session with runtime-inspect capability | OpenAPI 3.0.3 document. |

Open `http://localhost:8080/api/openapi/docs` in a browser to inspect the
contract shipped by the running binary. Treat that generated document as the
source for request and response schemas.

## Authentication

Authenticated HTTP routes expect:

```http
Authorization: Bearer <session-token>
```

The normal password flow starts at `POST /api/auth/login/start`. When passkeys
are disabled, a valid password login returns a token directly. When passkeys
are enabled, the response may require a WebAuthn finish step instead, so do not
assume that password-only scripts work in every deployment.

AgentHub does not currently expose a separate long-lived service-account token
contract. For unattended automation, keep the session creation and renewal
policy explicit, store tokens in a secrets manager, and expect a session to
expire after 12 hours.

## Fetch the Current Contract

With an authenticated token in `AGENTHUB_TOKEN`:

```bash
curl --fail \
  -H "Authorization: Bearer ${AGENTHUB_TOKEN}" \
  http://127.0.0.1:8080/api/openapi.json \
  -o agenthub-openapi.json
```

Inspect the path set before generating a client:

```bash
jq -r '.paths | keys[]' agenthub-openapi.json
```

The current schema focuses on Team workbench operations plus scoped object and
image uploads. General agent lifecycle routes may exist in the HTTP server
without appearing in the published OpenAPI document yet.

## Generate a Client

Pin both the AgentHub release and a copy or digest of its OpenAPI document. Do
not generate from a moving server during a release build.

### TypeScript

```bash
npx openapi-typescript agenthub-openapi.json -o src/agenthub-api.d.ts
```

### Python

```bash
openapi-generator-cli generate \
  -i agenthub-openapi.json \
  -g python \
  -o generated/agenthub_client
```

Generated clients still need the bearer session:

```python
from agenthub_client import ApiClient, Configuration

config = Configuration(host="https://agenthub.example.com")
config.access_token = token

with ApiClient(config) as client:
    pass
```

## Direct HTTP Example

Listing Teams is part of the published contract:

```bash
curl --fail \
  -H "Authorization: Bearer ${AGENTHUB_TOKEN}" \
  https://agenthub.example.com/api/teams
```

For create, run, step, mailbox, channel, goal-fork, and scoped-upload request
bodies, use the schemas embedded in `/api/openapi.json`. This avoids copying
large example payloads that drift as the contract evolves.

## Reliability Rules

- Set connection and request timeouts in every client.
- Retry transport failures and selected `5xx` responses with bounded backoff.
- Do not retry `400`, `401`, `403`, or `404` without changing the request or
  credentials.
- Treat `409` as a state/concurrency conflict and reload current state before
  deciding whether to retry.
- Supply the documented idempotency key on message operations that accept one;
  do not assume every POST is idempotent.
- Follow only pagination parameters declared by the operation. Agent history
  uses `before_id`; Team operations may use different shapes.

## Contract Verification

At deployment time:

1. Download `/api/openapi.json` from the exact candidate binary.
2. Compare its digest and path set with the reviewed contract.
3. Generate or compile the client against that file.
4. Run a read-only authenticated smoke request.
5. Exercise each mutating operation in staging with an isolated Team or agent.

The repository tests verify that documented OpenAPI paths are registered by the
HTTP router. They do not prove that routes omitted from the spec are stable for
external automation.

## Security

- Prefer HTTPS and private ingress for non-local deployments.
- Never put bearer tokens in committed files, command examples with real
  values, URLs, or CI logs.
- Grant automation only the role/capabilities it needs.
- Redact object source URLs and uploaded content from failure reports.

SSE uses a separate query-token transport for browser compatibility. See
[Connection Status and Recovery](./connection-status-and-recovery.md) before
building a streaming client.
