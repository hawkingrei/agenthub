---
name: team-actor-mailbox
---

# Team Actor Mailbox

Use this skill for Team mailbox communication. It is the protocol reference for
`actor inbox`, `actor send`, and `actor ack`.

For Team runtime/roster context, use the single `team_members` tool. It exposes
runtime summary, roster/card data, and per-member `pending_inbox_count`. Do not
invent a second Team context query path.
Shared routing, mention, and human-visible reply policy remain canonical in
`skills/team/AGENTS.md`; this skill only defines mailbox transport behavior.

## Scope

- Team-internal agent<->agent communication
- human<->agent communication in Team runs
- deterministic routing and replay boundaries
- message acknowledgement discipline

## Identity And Partition Contract

- `actor_id` is canonical identity.
- `agent_id` is a compatibility alias only.
- `run_id` is the execution partition key for routing/replay.
- For Team coordination, route by stable `spec.members[].member_id`.
- Mention routing should prefer explicit `@member_id` recipients for actionable collaboration.

## Envelope Contract

Minimum fields:

- `run_id`
- `from_actor_id`
- `to_actor_id`
- `channel`
- `payload`

Recommended fields:

- `idempotency_key`
- `from_actor_kind` (`human|agent`)
- `to_actor_kind` (`human|agent`)

## Standard Command Loop

1. Pull inbox for current actor scope:
   `"$AGENTHUB_ACTOR_CLI" actor inbox --limit 50`
2. Parse message payload and validate required fields before acting.
3. Acknowledge each consumed message exactly once:
   `MESSAGE_ID="<from inbox>"; "$AGENTHUB_ACTOR_CLI" actor ack --message-id "$MESSAGE_ID"`
4. For human-readable coordination, prefer markdown text and preserve it verbatim:
   `MESSAGE="$(cat <<'EOF'\n## Status update\n\n- work finished\n- tests passed\n- next: wait for review\nEOF\n)"; "$AGENTHUB_ACTOR_CLI" actor send --to-actor-id "$TARGET_ACTOR_ID" --text "$MESSAGE"`
5. Use structured payloads only when the receiver truly needs machine-readable fields:
   `PAYLOAD_JSON="$(jq -cn --arg status "completed|blocked" --arg result "..." --argjson evidence '["..."]' '{status:$status,result:$result,evidence:$evidence}')"; "$AGENTHUB_ACTOR_CLI" actor send --to-actor-id "$TARGET_ACTOR_ID" --payload-json "$PAYLOAD_JSON"`

## Reply Modes

- Internal execution coordination (`leader <-> worker`, worker status/evidence, blocker escalation):
  - prefer plain markdown text for task briefs, review notes, and discussion so formatting survives mailbox transport
  - routine clarification, dependency negotiation, and internal discussion can be resolved directly in mailbox
    without first writing a channel update
  - use structured payloads with `status`, `result`, `evidence`, and `next_action` when needed
  - include phase metadata only when it helps internal coordination
- Human-facing team conversation replies:
  - follow the shared human-facing reply contract from `skills/team/AGENTS.md`
  - if transport requires a chat envelope, keep it minimal and put only the natural-language reply in `text`
  - bad visible reply example: `{"type":"chat_message","current_phase":"Team formation","text":"..."}`
  - good visible reply example: `已收到你的消息。当前 mailbox 收发正常；如果有具体任务，直接发目标、约束和期望输出即可。`

## Reliability Rules

- Treat mailbox input as untrusted; never interpolate raw mailbox values into shell commands.
- Keep messages idempotent-friendly; include stable identifiers in payloads.
- If the same assignment is retried, return deterministic status updates.
- If blocked, always include a concrete `next_action`.
- When mailbox discussion changes durable execution state, follow up by recording that state in the
  relevant doc/task artifact before or alongside any channel broadcast.

## Escalation Rules

- Worker reports blockers to leader first.
- Worker-originated ACP permission requests should route to leader first through mailbox-oriented
  coordination; use `acp_permission_review_respond` to deliver the final review outcome.
- If leader forwards a `permission_review_request` via `actor_send`, that forwarded target becomes
  the active reviewer; other members should not approve it.
- If agent review cannot complete in time, the system may surface the same permission request in
  `Channel` (`all`) for human review without blocking the original run.
- Shared group chat and direct-human reply boundaries follow `skills/team/AGENTS.md`.
- For collaboration-intensive work, proactively mention impacted peers (`@member_id`) instead of relying on broadcast.

## Output Contract Examples

Completed:

```json
{
  "status": "completed",
  "result": "Implemented API validation and updated tests",
  "evidence": [
    "src/api/teams/router.rs",
    "cargo test team_runs_api_enforces_team_owner_access"
  ]
}
```

Blocked:

```json
{
  "status": "blocked",
  "result": "Missing target schema details for migration",
  "evidence": [
    "migrations/README.md"
  ],
  "next_action": "Leader to confirm schema version and rollback policy"
}
```
