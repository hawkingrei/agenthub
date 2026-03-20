# Team Workspace Language Cleanup

## Summary

- Removed the duplicate `all / Kanban` workspace tabs from the right-side Team header.
- Kept `Channel / Kanban` navigation in the left rail as the single primary navigation surface.
- Kept `Channel` as the left-rail surface group and rendered `all` as a concrete channel item (`# all`).
- Rendered the shared thread workspace heading as the concrete channel name `# all`, matching the model that `all` is one channel inside the broader channel surface.
- Moved the Team meta/actions menu from the right-side workspace header onto the existing Team name in the left sidebar, so Team metadata only appears on demand and does not add more always-visible header weight.
- Moved `Runs / Advanced` out of the left rail and into the workspace top-right utility area so they behave like operational views instead of competing subject navigation.

## Why

- The Team workbench had two competing primary navigations: the left rail and the right-side header tabs.
- `Runs / Advanced` are utility/execution views, not peer subjects beside `Channel / Kanban`.
- Keeping the left rail as `Channel -> # all` is clearer than repeating transport-heavy `all / Kanban` tabs inside the main workspace.
- The Team name was rendered in both the left rail and the workspace header, which added duplicate visual weight without adding information.
- The Slock / Notion direction is clearer: keep navigation nouns stable, and treat the concrete thread name as secondary metadata rather than the top-level page title.

## Validation

- `cd web && npm run lint -- src/pages/team_sidebar.tsx src/pages/team_page.tsx src/pages/team/use_team_runtime_effects.ts src/pages/team/use_team_runtime_effects.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npx vitest run src/pages/team/use_team_runtime_effects.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run build`

## Follow-up

- Verify on the deployed Team workbench that the left rail shows `Channel` with `# all` as the concrete item, the main workspace heading is `# all` (without duplicate `all / Kanban` tabs), `Runs / Advanced` only appear in the workspace top-right utility area, and Team metadata/actions only appear from clicking the Team name in the sidebar.
