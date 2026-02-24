# Workspace Output Interaction Refresh

## Background
The workspace output area needed clearer context and stronger interaction affordances, especially when the agents list stays collapsed by default. The goal was to make output state and navigation obvious without changing backend behavior.

## Scope
- Output header now summarizes agent status, session, last update time, and thinking runtime.
- Jump-to-bottom control surfaces pending output when the view is not following the tail.
- Terminal output auto-follow is tracked client-side with a pending count.
- Agents panel defaults to a collapsed rail with quick open/create controls.
- Debug tab sections are split into tag-based panels (session controls, permissions, raw events).
- No API or backend changes.

## Key Decisions
- Terminal follow state is derived from scroll position and output length, keeping the logic local to the client.
- Resume reuses ACP conversation jump and terminal scroll-to-bottom to avoid new data flows.
- ACP header badges are consolidated into the output header to remove duplicate status/session labels.

## Validation
- Run `npm test` in `web`.
- Manual checks:
  - Workspace loads with the agents rail collapsed.
  - Resume button appears after scrolling up in terminal output and clears after resuming.
  - ACP conversation shows resume when unfollowed and pending count increments.
  - Output header shows status, mode, session, and update time for the active agent.
