# ACP Search Fold, Plan View, and Input History

## Summary

Improve ACP conversation usability for long sessions by:

- forcing stale tool calls to auto-collapse when run status is terminal,
- adding a richer visual plan card in conversation mode,
- adding visible input history with keyboard recall in the docked input,
- and reducing repeated render cost with ACP markdown/ANSI render caches and
  `requestAnimationFrame` scroll throttling.

## Background

Three interaction gaps were reported in daily usage:

- `Tool Call: Search` could remain expanded when run status already ended,
- plan blocks were plain text and hard to scan under long outputs,
- users could not quickly inspect or reuse previously sent commands.

## Scope

- `web/src/components/acp_conversation.tsx`
- `web/src/conversation.ts`
- `web/src/components/input_dock.tsx`
- `web/src/app.tsx`
- `web/src/styles.css`
- `web/src/input_history.ts`
- `web/src/acp_conversation.test.ts`
- `web/src/acp_conversation_render.test.tsx`
- `web/src/input_history.test.ts`
- `docs/todo.md`

## Key Decisions

- Tool call live-state now combines tool status and run status:
  - if run status is terminal (`completed` / `failed` / `cancelled` / `stopped`),
    tool folds are treated as finished even when stale status still says
    `in_progress`.
- Tool call body now uses nested folds for `content/input/output/terminal` to
  reduce scroll pressure in explore/search-heavy traces.
- Plan rendering now shows a structured plan card:
  - progress summary (`completed / active / pending`),
  - visual progress bar,
  - per-step status and priority badges.
- Input dock now supports command history:
  - `History` menu shows recent sent commands,
  - `ArrowUp` / `ArrowDown` recalls history in textarea (with cursor-position
    guards to avoid breaking multi-line editing).
- History persistence uses local storage key `agenthub_input_history` and keeps
  a bounded de-duplicated list.
- Input history privacy guard now skips obvious secret-like commands from
  persistence (`password`/`token`/`api_key` assignments, bearer headers, and
  private key blocks).
- Input history menu behavior now closes on typing, history arrow navigation,
  `Escape`, and outside click.
- Input keyboard handling now includes explicit IME guards:
  - `nativeEvent.isComposing` and keyCode `229` are treated as composing state,
  - history navigation key handling is extracted to pure decision helpers with
  unit coverage.
- ACP conversation render hot path now includes cache layers:
  - markdown render cache keyed by message text,
  - ANSI segment parse cache keyed by terminal payload text.
- Conversation scroll handler now batches work with `requestAnimationFrame` to
  avoid bursty state updates under fast scroll.
- `requestAnimationFrame` throttling now uses a reusable helper
  (`createRafThrottle`) with unit coverage for dedupe and cancel behavior.
- ACP conversation row rendering now uses memoized row components:
  - conversation rows are split into a memoized `ConversationBubble`,
  - tool/plan/markdown bubbles are memoized to avoid repeated render work
    during viewport-only scroll updates.
- ACP conversation now applies virtual list slicing in non-stick mode:
  - render only viewport + overscan items,
  - keep top/bottom spacers to preserve scroll geometry,
  - preserve global item offset for stable collapse and ordering behavior.

## Validation

```bash
cd web
npm run test -- src/hooks/use_acp_conversation.test.ts src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/input_history.test.ts
npm run test -- src/input_dock_keyboard.test.ts src/raf_throttle.test.ts
npm run test -- src/input_history.test.ts
npm run lint -- src/components/acp_conversation.tsx src/components/input_dock.tsx src/input_history.ts src/input_history.test.ts src/conversation.ts src/app.tsx
npm run build
```

```bash
cargo test --test web_assets
```

## Follow-ups

- Add optional session-scoped history mode (per agent/session) if global history
  becomes noisy for multi-agent workflows.
- Add quick clear/remove actions in history menu when command volume grows.
