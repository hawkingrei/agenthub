# Team Create Mission-First Add Agent Cleanup

## Summary

This checkpoint tightens the default Team setup flow after shell-first Team creation. The empty-Team setup surface now presents the two intended participant paths, `Create New Agent` and `Copy Existing Agent`, while the first added agent remains the coordinator by default.

## Background

The canonical contract in `docs/features/team-create-flow.md` says Team creation should collect mission and identity first, then add participants afterward. It also says the default add-agent surface should avoid a coordinator/worker role switch and should keep copy semantics separate from a later move flow.

## Scope

- Rename the default new-agent entry from role-specific add labels to `Create New Agent`.
- Keep the create-agent modal title role-aware, but make its action label generic and its body describe a fixed Team assignment rather than a role-selection ceremony.
- Rename the existing-agent path to `Copy Existing Agent`.
- Remove the disabled `Move to Team (later)` action from the copy modal so move does not appear as a third default add-agent path.
- Keep the move warning as non-actionable context because ownership transfer still needs separate runtime and history guardrails.

## Key Decisions

- The two default add-agent paths are product choices, not role-management choices.
- The modal can still show whether the created/copied agent will become coordinator or worker, but the user does not choose that assignment in the default flow.
- Copy remains configuration-only for now; move is explicitly not part of the default modal action set.

## Validation

```bash
cd web && npm exec vitest -- run src/pages/team_setup_panel.test.tsx src/pages/team/team_management_modals.test.tsx src/pages/team_panels.test.tsx
cd web && npm run lint
cd web && npm exec tsc -- --noEmit
cd web && npm run build
```

## Follow-Ups

- Browser-check the create Team shell, first coordinator creation, and later worker creation on a narrow viewport before closing the full `P0+` Team create-flow backlog item.
- Keep the separate Team adoption work focused on copy-first behavior, optional workspace-content copy, memory/context seeding, and guarded move semantics.
