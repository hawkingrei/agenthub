---
sidebar_position: 2
---

# OpenAPI and Automation

AgentHub exposes authenticated OpenAPI discovery endpoints for integration and
automation workflows.

## Endpoints

- JSON spec: `/api/openapi.json`
- Lightweight docs page: `/api/openapi/docs`

Both endpoints require authentication.

## Fetch Spec With Token

```bash
curl -H "Authorization: Bearer <token>" \
  http://localhost:8080/api/openapi.json
```

Replace `<token>` with a valid AgentHub auth token.

## What You Can Automate

- Team definitions and runs
- Step lifecycle operations
- Actor mailbox send/inbox/ack flows
- API contract checks in CI

## Integration Pattern

1. Login once and get auth token.
2. Pull OpenAPI spec.
3. Generate typed client or API test stubs.
4. Use generated client in scripts or internal tools.
5. Re-sync spec after AgentHub upgrades.

## Reliability Tips

- Treat OpenAPI as the source of truth for payload shapes.
- Pin generated client version per release.
- Add contract tests for high-risk API paths.

## Related Pages

- [Team Workbench](./team-workbench.md)
- [Deployment Overview and Topology](../deployment/overview-and-topology.md)
