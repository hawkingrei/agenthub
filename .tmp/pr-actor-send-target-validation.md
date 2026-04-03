## Summary

- reject direct `actor send` targets that are not canonical Team `spec.members[].member_id` values
- keep human mailbox targets (`user`, `user:<id>`) valid
- enforce the rule on the mailbox service path, not in CLI-only preflight logic
- move `create_agent` / `send_input` request validation further up to the `/api/agents` HTTP boundary
- document and backlog the post-merge verification path

## Background

Mailbox investigation on Team run `737faf97-31c8-4ad2-8669-7b124a720541` showed that worker message `2148` was not lost. It was persisted as:

- `from_actor_id = c319f933-1358-4418-a111-872304052422`
- `to_actor_id = leader`
- `status = pending`

That Team's canonical leader member id is `595d1ae8-fcbd-4111-b5c7-d446a12c044b`. Direct messages to the UUID leader member were delivered normally; only the role alias `leader` remained unread forever. The direct-send path accepted the alias verbatim and the mailbox layer persisted it verbatim.

## What Changed

### Direct Mailbox Targets

- add direct mailbox target validation in `src/team/manager/mailbox.rs`
- resolve Team context from `run_id` on the service side before writing mailbox records
- reject non-human direct targets unless they match Team `spec.members[].member_id`
- return a targeted hint when the caller used a role alias such as `leader`
- add focused mailbox validation tests for:
  - role alias rejection
  - canonical member id acceptance
  - human mailbox acceptance
- add a gRPC regression that proves server-side `actor_send` rejects a role alias target
- remove the CLI-side preflight validation so the rule lives in one place
- update `actor send` help text to document the canonical member-id rule

### Agents API Boundary Validation

- normalize and validate `POST /api/agents` request fields in `src/api/agents.rs`
- reject blank `name` / `command`
- stop silently coercing unknown `worktree_mode` values to `use_existing`
- reject blank optional `worktree_repo` / `worktree_ref` values if callers provide them
- enforce the same `agent_loop.prompt` / `agent_loop.idle_seconds` enablement rules already used by the dedicated `agent_loop` route
- reject blank `POST /api/agents/:id/input` payloads before they reach the agent manager
- trim and validate optional `message_id` / `session_id` values instead of letting blank strings fall through
- add a new journal + TODO verification entry for the tightened `agents` API contract

## Validation

- `cargo test -p agenthub validate_direct_mailbox_target_ -- --nocapture`
- `cargo test -p agenthub grpc_actor_send_rejects_role_alias_target_on_server -- --nocapture`
- `cargo test -p agenthub parse_send_generates_default_idempotency_key -- --nocapture`
- `cargo test -p agenthub create_agent_route_rejects_blank_name -- --nocapture`
- `cargo test -p agenthub create_agent_route_rejects_blank_command -- --nocapture`
- `cargo test -p agenthub create_agent_route_rejects_invalid_worktree_mode -- --nocapture`
- `cargo test -p agenthub create_agent_route_validates_agent_loop_when_enabled -- --nocapture`
- `cargo test -p agenthub send_input_route_rejects_blank_input_and_identifiers -- --nocapture`
- `cargo test -p agenthub parse_worktree_mode_ -- --nocapture`
- `git diff --check`
