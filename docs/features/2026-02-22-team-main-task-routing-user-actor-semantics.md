# Team Main Task Routing And User Actor Semantics

## Background

The chat-first Team roadmap requires deterministic conversation semantics before
run compilation. We already persisted main-task conversations and route fields,
but actor and route contracts were still loose:

- `from_actor_id` accepted arbitrary values without team/user validation.
- `to_leader` and `group_chat` did not enforce deterministic `to_actor_id`
  behavior.
- Human user identity was represented as plain `"user"` instead of a canonical
  actor identity bound to the authenticated session.

## Scope

- Harden main-task actor semantics in `src/api/teams.rs`:
  - canonicalize user actor as `user:<authenticated_user_id>`
  - accept `"user"` alias but normalize to canonical value
  - reject mismatched `user:<id>` impersonation attempts
- Enforce route-specific target rules for main-task messages:
  - `to_member`: requires valid `to_actor_id` in `spec.members`
  - `to_leader`: resolves to leader member deterministically (auto-fill when omitted)
  - `group_chat`: requires `to_actor_id` to be omitted
- Restrict message sender to:
  - canonical authenticated user actor, or
  - a member from `spec.members`
- Extend API tests (`tests_core`, `tests_router`) to verify:
  - actor canonicalization
  - route rule enforcement
  - ordered replay by `message_id`

## Key Decisions

1. Keep user actor first-class but backward compatible.

- API still accepts `"user"` for client ergonomics.
- Server stores canonical `user:<id>` to avoid ambiguous actor identity.

2. Make routing deterministic at write-time.

- `to_leader` now resolves to a concrete leader member id before persistence.
- `group_chat` now stores `to_actor_id = null` by contract.

3. Validate against team spec at API boundary.

- Message sender/receiver are checked against `spec.members` (plus user actor)
  before writing to DB.

## Validation

Executed locally:

```bash
cargo test -q team_main_task_api_creates_lists_and_redacts_context -- --nocapture
cargo test -q team_main_task_messages_api_supports_route_and_redaction -- --nocapture
cargo test -q teams_router_http_contract -- --nocapture
cargo test -q team_main_task -- --nocapture
```

All commands passed.
