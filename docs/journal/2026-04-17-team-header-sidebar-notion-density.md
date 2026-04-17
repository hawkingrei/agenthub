# Team Header And Sidebar Notion Density

## Summary

- tightened the Team workspace header so the title/description read more like a page header and less like a tool bar
- lightened the runtime badge group and workspace action chrome
- reduced sidebar card-ness by lowering padding, border weight, and shadow weight across team/workflow/agent rows
- tuned sidebar search and section labels so the left rail reads closer to a Notion-style document index
- added regression assertions for the new header and sidebar density contracts

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team_panels.test.tsx src/pages/team/team_workspace_header.test.tsx`
- `cd web && npm run build`

## Notes

- Chrome DevTools MCP baseline/regression verification could not be completed in this environment because `chrome-devtools/list_pages` returned `Transport closed`.
- follow-up review cleanup replaced the invalid `pl-4.5` sidebar padding utility with `pl-5` and centralized the shared light chrome shadow token in `tailwind_classes.ts`.
- while re-running `web` build on the branch, an unrelated stale `qrcode` dependency path in `use_app_admin.ts` surfaced; the branch now uses token/PIN-only admin join output so the PR stays buildable on the current mainline dependency set.
