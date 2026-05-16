# Team Conversation Slock Polish

## Scope

This pass tightened the Team chat, thread, and ACP presentation so they read like one chat system instead of three adjacent surfaces:

- the main Team channel lane now uses a wider message content column instead of stopping early on a fixed reading width;
- human bubbles no longer rely on a strong blue fill and now keep a neutral, content-first tone;
- the thread pane now uses the same message-row rhythm as the main lane (`avatar + meta + bubble`);
- channel, thread, and ACP composers now share one lightweight shell/editor-row/send-button language;
- `markdown-it` rendering now emits stable `md-*` structure classes so rich text blocks can be styled as chat-native rich text instead of document cards.

## Key UI Rules Landed

### Message rows

- Prefer full-width content columns after the avatar lane.
- Keep author/time/meta visually lighter than message text.
- Use the same row-shell hover rhythm for Team, thread, and ACP messages.

### Bubbles

- Keep bubbles neutral and light; do not use heavy fills to encode author type.
- Keep seen/delivery affordances as small bottom-right overlays instead of inline numeric badges.
- Make thread root/reply bubbles and ACP message bubbles match the main Team lane closely enough that they feel like one product.

### Rich text

- Use `markdown-it` renderer rules to emit stable semantic classes:
  - `md-link`
  - `md-paragraph`
  - `md-blockquote`
  - `md-list`
  - `md-list-item`
  - `md-inline-code`
  - `md-code-block`
  - `md-table-wrap`
  - `md-table`
- Style rich text blocks as chat-native rich text, not embedded document cards.

### Composer system

- Channel, thread, and ACP composers should all read as one family:
  - light shell
  - shared editor row
  - rounded send action
  - low-emphasis helper text / secondary actions

## 2026-05-16 Team Composer Component Checkpoint

This follow-up moves the Team channel and Team thread composers from parallel
page-local markup to a shared `TeamMessageComposer` component. The goal is not a
visual redesign; it narrows drift by making both surfaces reuse the same shell,
editor row, mention menu, helper row, and send-button structure while leaving
channel/thread-specific submit semantics in their owning panels.

What changed:

- `TeamTaskPanel` now uses the shared composer for the channel lane.
- `TeamThreadPane` now uses the same composer for thread replies.
- shared composer class names now own the mention-menu and context-row styling;
  existing thread constants alias those shared names for compatibility.
- focused component coverage locks the shared shell and mention-selection
  behavior.

Browser note:

- local Vite loaded successfully at `http://127.0.0.1:4176/workspace/teams`,
  but the unauthenticated local session redirected to the login page, so this
  checkpoint does not close the deployed Chrome MCP validation item.

## Chrome DevTools MCP Baseline Notes

Chrome DevTools MCP was used against logged-in `app.slock.ai` surfaces during this pass to keep the target interaction language honest.

Observed baseline traits:

- message lanes prioritize content width over fixed reading columns;
- thread panes read like an extension of the main chat lane, not a separate tooling card;
- team/agent identity stays inline near the primary label instead of hiding all context in menus;
- input areas feel like light chat composers, not generic form panels;
- heavy author-color fills are avoided in favor of lighter message rows and calmer chrome.

Wrap-up MCP note:

- the final selected Slock page during wrap-up was a logged-in machine detail page, which still confirmed the same light header/sidebar/navigation tone;
- this pass did **not** include a final Chrome DevTools MCP regression on deployed AgentHub yet, so deployed validation remains open in `docs/todo.md`.

## Local Validation

Executed during this pass:

```bash
cd web && pnpm exec vitest run src/markdown.test.ts src/pages/team/team_markdown.test.ts src/pages/team_panels.test.tsx
cd web && pnpm exec vitest run src/acp_panel.test.tsx src/pages/team/team_thread_pane.test.tsx src/input_dock_render.test.tsx
cd web && npm run build
```

Additional validation for the 2026-05-16 composer checkpoint:

```bash
cd web && npm exec vitest run src/pages/team/team_message_composer.test.tsx src/pages/team/team_thread_pane.test.tsx
cd web && npm exec vitest run src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx
cd web && npm exec tsc -- --noEmit
cd web && npm run lint
cd web && npm run build
```

Latest local build snapshot at wrap-up:

- `route-teams`: `277.66 kB`

## Remaining Follow-up

- Run deployed Chrome DevTools MCP regression on `agenthub.hawkingrei.com` for:
  - Team channel lane
  - thread pane
  - ACP conversation
  - composer transitions between those surfaces
- Decide whether any remaining Team/ACP message-row constants should move into a smaller dedicated shared chat-style module instead of continuing to live inside page-local files.
