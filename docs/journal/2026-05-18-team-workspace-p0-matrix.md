# Team Workspace P0 Matrix

## Summary

This checkpoint tightens the Team workspace route selection, channel/thread
layout, search-command flow, and a small shared-primitive composition slice.

The deployed browser pass against `agenthub.hawkingrei.com` is covered for the
Team workspace layout and search-command matrix. Authentication required
injecting the current local AgentHub auth session into the browser because the
initial browser profile did not carry an `agenthub.hawkingrei.com` login state.

## Background

The active P0 backlog had two related web gaps:

- Team workspace browser behavior for channel/thread split and independent
  search command flow.
- Continued Team workspace composition cleanup and primitive consolidation.

## Scope

- Treat `?lens=search` on Team detail routes as a deprecated URL shape that
  resolves back to the channel surface.
- Keep Team sidebar search as a command dialog trigger, not a normal content
  lens navigation action.
- Let channel content use the full lane until a thread is opened, then split
  channel and thread panes evenly with the thread dock allowed to consume the
  available height.
- Replace two local empty-card assemblies with the shared `EmptyState`
  primitive and remove the associated class plumbing from
  `TeamWorkbenchContent`.

## Key Decisions

- Search remains a command-level interaction. Team routes may still tolerate
  old `?lens=search` URLs, but they no longer make the right-side content render
  a standalone Search lens.
- Thread is still a subordinate channel detail, not a top-level workspace lens.
  The adaptive behavior is layout-only: no open thread means the channel lane
  owns the full center surface; an open thread introduces the split.
- This checkpoint does not close the broader Team composition P0. It lands one
  bounded primitive/construction slice while leaving deeper panel, dock, toolbar,
  and row migrations open.

## Validation

```bash
npm exec vitest -- run src/app_route_selection.test.ts src/pages/team/team_workbench_content.test.tsx src/pages/team_panels.test.tsx src/pages/team/use_team_workspace_view_model.test.tsx
npm run test -- src/app_route_selection.test.ts
npm run lint
npm exec tsc -- --noEmit
make build-web
git diff --check
```

The `npm run test -- src/app_route_selection.test.ts` pass also confirmed the
localStorage warning is gone from the npm test entrypoint. Node still emits the
separate `DEP0205 module.register()` deprecation warning.

Browser deployment checks:

```bash
agent-browser --session-name agenthub-team-ui open https://agenthub.hawkingrei.com
```

- Desktop channel-only state on
  `/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`: channel lane used
  the full 1460px center content width, with no thread dock and no horizontal
  overflow.
- Desktop open-thread state on the same team: channel and thread panes split
  evenly at 724px each with a 12px gap, `thread-dock` consumed the full
  available height, and no horizontal overflow was present.
- Mobile `390x844` open-thread state: channel and thread stacked vertically,
  both used the available 386px content width, and document horizontal overflow
  stayed false.
- Search command dialog did not change the route when opened, filtered
  channels/tasks/agents locally, and selecting results updated the route and
  right-side content:
  - `Kanban` -> `?lens=tasks`
  - `# all` -> default Team detail channel URL
  - `tidb-fuzz-bugfix-team-worker-2` ->
    `?lens=members&member=2b71c038-ce49-4f82-9732-0b387a18bf31&tab=thread`
- Direct left-sidebar `Tasks` and `Agents` clicks selected default content and
  updated the right-side panel instead of leaving it stale.

## Follow-Ups

- Record CI run IDs after this branch is pushed.
- Continue moving remaining Team panels, docks, toolbars, and row/list
  affordances onto shared primitives.
