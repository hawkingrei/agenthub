# Web Tailwind UI Phase-9: ACP Conversation Shell

## Background

After phase-8 (`AcpPanel` / `AcpDebug`), the conversation surface still relied on
legacy global CSS for most visual hierarchy. This phase migrates the ACP
conversation shell styling to Tailwind utilities while preserving rendering and
interaction semantics.

## Scope

- `web/src/components/acp_conversation.tsx`
- `docs/todo.md`

## Supersession

Stable ACP conversation-shell, fold, payload, progressive rendering, and semantic-class rules from
this note now live in `docs/features/acp-runtime.md#4-conversationdebug-surfaces` and
`docs/features/frontend-design.md#42-acp-heavy-output-visual-contract`. This journal remains the
rollout evidence for the Tailwind phase-9 ACP conversation migration.

## Key Decisions

1. Keep behavior unchanged:
   - fold open/close logic for tool calls and nested sections
   - payload filtering rules (debug field hide, output priority)
   - progressive "Show more" rendering and virtualization spacers
2. Preserve semantic classes required by tests and legacy selectors:
   - `acp-tool-fold`, `acp-tool-group-fold`, `acp-subfold`
   - `acp-payload-*`, `acp-diff-*`, `acp-thinking-title`, `acp-plan-card`
3. Apply Tailwind only as visual layering:
   - conversation viewport and row spacing
   - bubble tone/card borders for user/agent/tool/plan messages
   - fold summary readability and status chips
   - payload card/list/grid readability

## Validation Evidence (local)

- Focused render/interaction tests:
  - `npm --prefix web run test -- src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/conversation.test.ts`
- ACP + Team/Agents regression tests:
  - `npm --prefix web run test -- src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx src/agents_panel.test.tsx src/output_header.test.tsx src/output_body.test.tsx src/input_dock_render.test.tsx src/input_dock_keyboard.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/acp_panel.test.tsx src/acp_debug.test.tsx src/acp_debug.interaction.test.tsx src/acp_debug_permissions.test.ts`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in ACP mode:
  - conversation scroll comfort and top-hint readability
  - focused tool-call highlight and fold summary readability
  - payload nested/object/list readability under long outputs
  - plan card progress/readability in mixed run states

## Notes

- During migration, class-token changes were kept compatible with existing
  render tests that assert hidden debug values are absent from HTML snapshots.
- Follow-up adjustment (same day): tool-call visual classes were narrowed back to
  semantic classes (`acp-bubble.tool_call`, `acp-tool-fold`, `acp-tool-group-*`,
  `acp-subfold`) so legacy `styles.css` remains the primary source of truth for
  tool-call tones and fold presentation.
