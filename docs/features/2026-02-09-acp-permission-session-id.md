# ACP Permission Session IDs

## Background
ACP permission requests were persisted with the ACP session ID in the `session_id` column, but the database enforces a foreign key to `agent_sessions(id)`. This mismatch causes foreign key failures and drops permission requests.

## Scope
- Store the AgentHub agent session ID in `acp_permission_requests.session_id`.
- Add `acp_session_id` to preserve the ACP session ID for debugging.
- Keep API responses backward compatible by making the new field optional.

## Key Decisions
- Keep the foreign key constraint intact for consistency.
- Add a nullable `acp_session_id` column via `ALTER TABLE` and include it in new inserts.

## Validation
- Start a new ACP session and trigger `/permission-demo`.
- Confirm the permission request persists without foreign key errors.
- Verify Debug -> Raw Events and the permission modal show the pending request.
