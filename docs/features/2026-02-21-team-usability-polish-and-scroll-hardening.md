# Team Usability Polish And Scroll Hardening

## Background

Recent Team UI changes fixed overlap and collapse regressions, but daily usability still had friction:

- member strip in `TeamRunPanel` showed only `L/W + dot`, making member identity hard to scan;
- run input area lacked direct affordances for valid JSON examples/clear action;
- right pane scroll behavior could feel clipped when `Active Run` content became long;
- legacy Team grid/panel blocks in `styles.css` still overlapped with component Tailwind intent.

## Scope

- `web/src/pages/team_run_panel.tsx`
- `web/src/pages/team/create_helpers.ts`
- `web/src/pages/team/state.ts`
- `web/src/pages/team_sidebar.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `web/src/ui/tailwind_classes.ts`
- `web/src/styles.css`
- `web/src/pages/team_panels.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Improve Team member strip readability in run panel:
   - keep role badge (`L/W`) and lifecycle dot;
   - add visible member ID (mono, truncation-safe) and lifecycle status chip per member.
2. Improve run creation input ergonomics:
   - add explicit context-id hint text;
   - add JSON quick actions (`Use Example JSON`, `Set Empty Object`, `Format JSON`, `Clear`);
   - add inline JSON parse validation feedback and disable `Create Run` when input is invalid;
   - add `Ctrl/Cmd + Enter` shortcut to submit a run directly from the JSON editor;
   - set default run input draft to `{}` to encourage direct structured editing;
   - keep empty input semantics explicit (`{}` as default backend input).
3. Harden right-pane scrolling:
   - make `teams-layout` consume available vertical space (`flex-1`);
   - make `teams-main` explicitly scrollable (`overflow-y-auto`) so `Active Run` + tabs are not visually clipped.
4. Further reduce style ownership conflicts:
   - move mailbox advanced panels to explicit Tailwind layout/surface classes;
   - remove redundant legacy Team grid/panel visual overrides in `styles.css` that competed with Tailwind component classes.
5. Keep test selectors stable:
   - preserve semantic class hooks (for example `teams-run-create`, `teams-step-panel`, `teams-message-panel`) while moving visual ownership to Tailwind utilities.

## Validation

- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team/state.test.ts`
- `npm --prefix web run lint`
- `npm --prefix web run build`
- Chrome DevTools MCP snapshot check on `/teams`:
  - verified `Create Run` controls are present and stable;
  - verified `Active Run` section and tabs are visible in the right pane;
  - verified no duplicate/overlapped Team shell blocks appeared in the checked snapshot.

All checks passed locally after this usability pass.
