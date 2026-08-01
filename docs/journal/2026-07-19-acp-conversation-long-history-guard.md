# ACP Conversation Long-History Guard

## Summary

ACP conversation long-history behavior now has explicit hook-level regression coverage for its two-stage rendering strategy: while pinned to the bottom, long conversations expose a bounded recent tail window; after a real upward scroll detaches from the bottom, the hook restores the full source list and uses viewport virtualization for the visible slice.

## Background

ACP conversation rendering already had spacer-based virtualization and recent-tail primitives. The frontend performance TODO still needed stronger evidence that the two behaviors are not accidentally collapsed into one mode during refactors. The risk is subtle: always rendering the full list while pinned can inflate DOM cost, while always tail-windowing after user scroll would hide older context from an operator who intentionally moved upward.

## Scope

- Added assertions to `web/src/hooks/use_acp_conversation.interaction.test.tsx` for the default pinned long-history state.
- Verified the same fixture switches to full-source virtualization after the user scrolls upward.
- Kept production ACP hook behavior unchanged.

## Key Decisions

- Treat pinned-tail and scroll-up virtualization as distinct UI states.
- Keep the evidence at the hook boundary because `useAcpConversation` feeds both Agents ACP and Team member ACP panels.
- Do not mark the broader frontend performance TODO complete from this slice alone; other Team surfaces and browser/profiler evidence still need their own coverage.

## Validation

Targeted checks for this slice:

```bash
cd web && npm exec vitest -- run src/hooks/use_acp_conversation.interaction.test.tsx src/hooks/use_acp_conversation.test.ts src/conversation_window.test.ts
```

## Follow-Ups

- Continue long-history audits for remaining high-volume Team surfaces.
- Add browser/profiler evidence before making broad page-level responsiveness claims.
