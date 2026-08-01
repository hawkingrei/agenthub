# Team Conversation Style Convergence

## Summary

The ACP conversation, Team thread pane, and Team mailbox now use one neutral message-bubble base
style. Channel and thread composition continue to use `TeamMessageComposer`, while ACP keeps its
own input-dock behavior and consumes the same composer row styles.

## Background

The workspace uses several conversation entry points with different delivery, history, and runtime
controls. Their visual language had converged for composition, but mailbox outgoing messages still
used accent-colored bubbles that made the surface read as a different system.

## Scope

- Extract the neutral bubble base classes into a shared frontend token.
- Apply the token to ACP, thread, and mailbox message bubbles.
- Preserve each surface's spacing, alignment, history, interrupt, and delivery behavior.

## Key Decisions

- Reuse a style token only; do not merge ACP input-dock behavior into the Team message composer.
- Keep the mailbox's directional alignment and corner treatment while removing outgoing accent
  fills and borders.

## Validation

```bash
cd web && npm run test -- acp_conversation_render.test.tsx pages/team_panels.test.tsx pages/team/team_thread_pane.test.tsx components/input_dock.test.tsx
cd web && npm run lint
cd web && npm run build
git diff --check
```

## Follow-Ups

- Capture authenticated deployed-browser evidence across the workspace conversation flow.
