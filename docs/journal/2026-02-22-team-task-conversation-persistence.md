# Team Main Task Conversation Persistence

## Background

The chat-first Team roadmap requires durable artifacts for "discussion before
execution": task records, conversation threads, and message history.
Before this change, Team data focused on run/step/mailbox execution artifacts,
without a dedicated persistence model for user-leader negotiation state.

## Scope

- Add explicit Team task/conversation persistence models in domain and DB:
  - `team_tasks`
  - `team_conversations`
  - `team_conversation_messages`
- Add Team manager APIs for:
  - task creation/list/get
  - conversation lookup
  - conversation message append/list
- Add HTTP APIs for chat-first flows:
  - `POST/GET /api/teams/:id/tasks`
  - `GET /api/teams/:id/tasks/:task_id`
  - `POST/GET /api/teams/:id/tasks/:task_id/messages`
- Enforce payload/context redaction for sensitive keys before SQLite persistence.

## Key Decisions

1. Keep run orchestration and task conversation as separate layers:
   - task stores planning/negotiation artifacts
   - run stores execution artifacts
2. Use explicit redaction at manager boundary to avoid accidental secret writes:
   - redact key paths containing:
     `token`, `secret`, `password`, `authorization`, `api_key`, `apikey`
3. Keep API contracts deterministic with constrained routing/mode enums:
   - conversation mode: `to_leader`, `to_member`, `group_chat`
   - message route: `to_leader`, `to_member`, `group_chat`
4. Team deletion now cascades across new conversation tables to avoid orphaned
   dialogue records.

## Validation

Executed locally:

```bash
cargo test -q task_and_conversation_messages_are_persisted_with_redaction -- --nocapture
cargo test -q team_task -- --nocapture
cargo test -q team_task_messages_api_supports_route_and_redaction -- --nocapture
cargo test -q teams_router_http_contract -- --nocapture
```

All passed.

## Follow-up

- Implement roadmap phase "leader plan compiler" so negotiated task context
  can be deterministically translated into run payload/spec.
- Add UI-level chat-first entry and replay surfaces for task/conversation
  APIs (with actor routing mode controls).
