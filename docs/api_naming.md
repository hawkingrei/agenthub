# API Naming Conventions

## Canonical Casing
AgentHub-owned JSON payloads (HTTP API + SSE) use `snake_case`.

## Boundary Normalization
Upstream protocols may emit `camelCase`. Normalize at the boundary:
- Parse `camelCase` input via serde aliases.
- Re-emit only `snake_case`.
- Store JSON blobs in `snake_case` (avoid dual-casing in storage).

## UI Types
Frontend types must only expose `snake_case` fields. Do not keep both casings in public interfaces.

## Example
`optionId` (upstream) -> `option_id` (AgentHub API/UI/storage).
