# P2P Review Follow-up: Audience And Agent-Node Ordering

## Summary

- aligned issued internal-node token metadata with the JWT audience claim
- stabilized local agent-node list updates so UI ordering matches backend ordering semantics

## Details

### Internal node credential audience

- `InternalAuthz::issue_node_access_token(...)` now derives a single token audience that is used consistently in:
  - JWT custom claims
  - returned `IssuedNodeAccessToken.audience`
- when `expected_audience` is configured, it remains authoritative
- when `expected_audience` is unset, the token uses the primary normalized request audience

### Agent-node UI ordering

- `upsertAgentNodeRecord(...)` no longer appends every created or updated node to the end
- updates replace the existing node in place
- inserts keep the same ordering contract as the backend:
  - `main` first
  - remote nodes by `created_at` descending
  - `id` as the deterministic tie-breaker

## Validation

- `cargo test issue_node_access_token_prefers_expected_audience_for_claims_and_metadata -- --nocapture`
- `cargo test issue_node_access_token_uses_primary_normalized_request_audience_without_expected -- --nocapture`
- `cd web && npx vitest run src/app.route_auth.test.ts`
- `cd web && npm run lint -- src/app.tsx src/app.route_auth.test.ts`
- `git -c core.fsmonitor=false diff --check`
