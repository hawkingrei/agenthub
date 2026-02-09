# API Naming Conventions

## Background
We observed inconsistent casing for the same field (`option_id` vs `optionId`) across ACP events, database storage, and web API responses. This caused client-side mismatches and approvals being treated as cancelled.

## Scope
- Define a single canonical casing for AgentHub-owned JSON payloads.
- Clarify how to handle upstream protocols (e.g., ACP) that use different casing.
- Document boundary mapping rules and expectations for storage and UI types.

## Key Decisions
- AgentHub public API and SSE payloads use `snake_case`.
- Database JSON blobs store `snake_case` fields to keep consistency with the API.
- Upstream protocols that emit `camelCase` must be normalized at the boundary (parse aliases, then re-emit `snake_case`).
- UI types only expose `snake_case` fields; do not carry dual-casing in public interfaces.

## Validation
- Review new API fields to ensure they follow `snake_case`.
- Confirm legacy `camelCase` payloads are normalized server-side.
- Run UI tests that validate permission option IDs use `option_id`.
