---
name: agenthub-acp-rendering
description: "Trigger when Team ACP UI looks blank, partial, duplicated, or stale, especially in `web/src/pages/team/` and ACP conversation surfaces."
---

# AgentHub ACP Rendering

Trigger when you are fixing AgentHub web ACP rendering behavior, especially under `web/src/pages/team/`, `web/src/components/acp_*`, and `web/src/hooks/use_acp_conversation.ts`.

## Scope

This skill is for:

- member ACP pages that look blank, busy, or stuck on loading
- chunked `agent_message` history and partial leading replies
- cache handoff between live state, render cache, and fetched ACP events
- duplicate ACP refreshes or overly aggressive polling
- Chrome DevTools MCP verification of real rendered ACP behavior

Do not use this skill for generic React styling work with no ACP/history behavior involved.

## Core Model

Treat ACP rendering as three separate concerns:

1. **Raw visible content**
   - what `buildAcpView(...)` and `buildConversationMessages(...)` can see before filtering
2. **Renderable content**
   - what remains after hiding an incomplete leading ACP message
3. **History recovery**
   - whether the UI should fetch one bounded older page to recover an incomplete leading reply

Never collapse these into one vague "has content" boolean.

## Rules

### 1. Distinguish partial-only from renderable

- A page with only a partial leading `agent_message` is **not** warm visible content.
- A page with a partial leading message plus a later complete reply is renderable.
- Warm cache and hydrated state must only count **renderable** content, not raw visible fragments.

### 2. Keep bounded recovery narrow

- For a partial leading ACP reply, auto-recover at most **one** older page unless there is a strong repo-local reason to widen it.
- Do not reopen deep `before_id=...` backfill loops.
- If the bounded recovery still cannot produce renderable content, prefer a clean empty state over rendering truncated text.

### 3. Prefer pure decision functions

- Put initial ACP history decisions into a pure function.
- The function should answer:
  - current ACP initial-history state
  - renderable vs visible counts
  - whether bounded history recovery should run
- Callers should consume the decision, not rebuild the logic with new ad-hoc booleans.

Good direction:

- `resolveInitialAcpHistoryDecision(...)`

Avoid:

- repeating partial/chunk/cache checks in `use_team_actions.ts`, `use_team_member_acp_view_model.ts`, and ACP panel code separately

### 4. Coalesce duplicate refreshes

- If the same `agentId + sessionId + mode=replace` request is already in flight, coalesce it.
- Selection-effect dedupe is helpful, but network-layer coalescing is the real safety net.

### 5. Prefer SSE-driven refresh over blind polling

- For `agent_acp` and `member_console`, updates should follow member SSE when possible.
- Do not keep refreshing full run/snapshot/event chains if ACP can be updated from member activity.
- Keep polling only as explicit fallback.

## Investigation Workflow

### 1. Reproduce with Chrome DevTools MCP

Before editing:

- open the real ACP page
- inspect the visible UI state
- inspect console
- inspect network for:
  - repeated `/api/agents/:id/events?...`
  - `before_id=...` deep backfill
  - run/snapshot/event polling noise

Record concrete evidence:

- page URL
- session id
- whether the page is blank, partial, or renderable
- exact request pattern

### 2. Match the real payload shape

Do not stop at a synthetic single-chunk fixture.

If the real response contains:

- high `chunk_index`
- many chunks for one `message_id`
- trailing `session_update`
- out-of-order event ids or mixed event types

then encode that shape into a focused regression test.

### 3. Fix the smallest stable boundary

Prefer this order:

1. pure decision/helper layer
2. fetch/recovery orchestration
3. view-model gating
4. final UI copy/chrome

Do not patch only the displayed text if the decision model is still wrong.

## Testing Expectations

Every ACP bugfix should add focused coverage for the real regression boundary.

Common targets:

- `web/src/pages/team/acp_history_prefetch.test.ts`
- `web/src/pages/team/use_team_actions.test.tsx`
- `web/src/pages/team_member_acp_panel.test.tsx`
- `web/src/hooks/use_acp_conversation.interaction.test.tsx`
- `web/src/acp.test.ts`

Preferred test style:

- table-driven tests for decision helpers
- focused hook tests for fetch/prefetch behavior
- panel/view-model tests for user-visible loading vs empty vs renderable states

## Validation

For ACP web changes, prefer this minimal validation set:

```bash
cd web && pnpm exec vitest run \
  src/pages/team/acp_history_prefetch.test.ts \
  src/pages/team/use_team_actions.test.tsx \
  src/pages/team_member_acp_panel.test.tsx
cd web && npm run build
```

Add `src/hooks/use_acp_conversation.interaction.test.tsx` when scroll/top-edge history loading changes.

After deployment, re-check the same page with Chrome DevTools MCP and confirm:

- blank/busy/partial symptom is gone
- duplicate ACP event fetches are gone or reduced as intended
- no new console errors were introduced
