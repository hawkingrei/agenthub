# Team Channel Conversation Tail Window

## Summary

Team channel timelines already use the shared Team conversation window helper to keep pinned long
histories bounded on first render. The channel activity list renders the recent visible tail while
stuck to the bottom, preserves a spacer for hidden earlier rows, and expands the full source list
after the user scrolls upward.

## Background

The frontend performance TODO still mentioned remaining explicit long-history behavior for Team
surfaces. Current code and tests show that the Team channel timeline has the same pinned-tail versus
scroll-up expansion boundary as the Team thread and mailbox conversation surfaces.

## Scope

- Document the existing Team channel long-history evidence.
- Narrow the remaining frontend performance TODO to browser/profiler evidence and any still
  uncovered edge surfaces.
- Avoid changing runtime or UI behavior in this checkpoint.

## Key Decisions

- `TeamTaskPanel` keeps channel activity rows bounded through `windowTeamConversation` while
  `stickToBottom` is active.
- Thread replies are still counted from the full ordered message list so a viewport window does not
  hide reply-count metadata for visible root messages.
- Markdown rendering and read-state precomputation stay scoped to visible channel rows in the
  pinned-tail state.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_panels.test.tsx
```

Relevant coverage:

- `TeamTaskPanel only renders markdown for the visible tail window until history is expanded`
- `TeamTaskPanel sticks to bottom by default and shows a jump action after manual upward scroll`

## Follow-Ups

- Broad frontend performance closure still needs browser or profiler evidence across full Team and
  ACP-heavy pages.
