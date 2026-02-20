# Team Member Dot Health And MCP Skill Display

## Background

The Team run workbench health block was too dense on narrow viewports: role/status/inbox/workline rows stacked heavily and reduced scan speed.
At the same time, team member MCP-related capability visibility in `Member Console` was implicit inside long `skills` text.

## Scope

- `web/src/pages/team_run_panel.tsx`
- `web/src/pages/team_member_console_panel.tsx`
- `web/src/styles.css`
- `web/src/pages/team_panels.test.tsx`

## Key Decisions

1. Simplify the Team health section into a compact member indicator strip:
   - Replace row-heavy `Team Health` details with `team_number=<total>` summary.
   - Render one member token per member with a dot + member id.
2. Use binary startup visual status for fast recognition:
   - `active` lifecycle => green dot.
   - non-active lifecycle (`inactive` / `missing`) => red dot.
3. Keep MCP-related visibility explicit in Member Console:
   - Add `mcp_skills` line derived from member skills matching MCP/runtime mailbox keywords.
4. Harden desktop overlap risk for long fields:
   - Member Console details moved to responsive card grid with wrapped long-value rendering.
   - Prompt moved into collapsible block to avoid forcing large always-open vertical text wall.
   - Add overflow guards for Team run meta cards and chat header rows to prevent long IDs from squeezing/overlapping neighboring UI.

## Validation Evidence (2026-02-20)

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`

## Notes

- This change is presentation-focused and does not alter team run lifecycle logic or mailbox semantics.
