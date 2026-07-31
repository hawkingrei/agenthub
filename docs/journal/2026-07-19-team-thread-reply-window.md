# Team Thread Reply Window

## Summary

Team thread panes now keep extremely long reply lists bounded on first render. When a reply thread is large enough to default to bottom-pinned behavior, the pane renders the recent tail window and exposes an explicit `Show earlier replies` action to expand the full thread.

## Background

The frontend performance hardening TODO already reduced avoidable rerenders for Team and ACP-heavy rows. Thread panes still needed an explicit long-history behavior beyond row memoization: a very large reply list could still create every reply row during the initial render even when the operator usually needs the latest replies first.

## Scope

- Reused the existing Team conversation viewport helper for thread replies.
- Kept short reply threads unchanged.
- Added a bounded recent reply window for large threads.
- Added an explicit expansion action so older replies remain reachable without changing backend history semantics.
- Kept the existing constrained scroll region, original root message rendering, composer behavior, and memoized row comparator.

## Key Decisions

- Use a dependency-free tail window instead of adding a virtual-list framework for the thread pane.
- Window only the replies, not the root message. The root remains visible as the thread anchor.
- Reset the expanded state when the selected root message changes, so each newly opened long thread starts from the bounded recent view.
- Keep this as a Team thread-specific long-history guard. Channel, mailbox, and ACP broad long-history behavior still need their own evidence before the frontend performance TODO can close.

## Validation

Targeted checks for this slice:

```bash
cd web && npm exec vitest -- run src/pages/team/team_thread_pane.test.tsx src/pages/team/team_conversation_viewport.test.ts
```

## Follow-Ups

- Broader frontend performance closure still needs explicit long-history behavior or browser/profiler evidence for the remaining high-volume Team/ACP surfaces.
