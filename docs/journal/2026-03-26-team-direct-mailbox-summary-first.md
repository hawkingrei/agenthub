# Team Direct Mailbox Summary-First

## Summary

- default Team task-message routing now prefers direct mailbox delivery when exactly one member is
  targeted and no explicit route was provided
- `chat_message` payloads can carry summary-first metadata through `summary` plus `detail_ref`
- Team prompts and mailbox skills now instruct agents to use direct mailbox first and to attach
  stable detail references instead of pasting large evidence into routine coordination messages

## Implementation Notes

- backend route inference now derives `to_member` / `to_leader` from a single mentioned member or
  explicit `to_actor_id` when the caller omits `route`
- backend payload normalization preserves `detail_ref` as normalized object metadata and derives a
  fallback `summary` from `text` / `result` when needed
- Team conversation web send no longer hard-codes `group_chat`; it lets the backend infer the most
  compact delivery path
- mailbox/prompt docs now describe `summary + detail_ref` as the preferred pattern for large
  evidence handoffs

## Validation

- `cargo test team_task_messages_api_infers_direct_route_for_single_mention_and_normalizes_detail_ref -- --nocapture`
- `cargo test team_task_messages_api_supports_route_and_redaction -- --nocapture`
- `cargo test -p agenthub-team-prompts prompt_templates_keep_required_contract_lines -- --nocapture`
- `cd web && npx vitest run src/pages/team/mailbox_helpers.test.ts src/pages/team/use_team_conversation_actions.test.tsx`

## Follow-up

- deployed verification should confirm single-mention Team conversation sends reduce mailbox fan-out
  in real sessions without breaking human-visible conversation history
- future work can add first-class artifact storage / browsing so `detail_ref` points at stable
  AgentHub-managed evidence instead of caller-owned URIs
