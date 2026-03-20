# Team runtime updates no longer render as warning alerts

## Summary

`Teams` previously rendered both real warnings and successful runtime control summaries through the
same yellow `Alert` surface. That made messages such as `Team runtime updated (started=3)` look
like an error even though the operation succeeded.

This change separates those two cases:

- runtime start/stop summaries now render as a lightweight success/info notice;
- actual warnings still render through the warning `Alert` path.

## What Changed

- `web/src/pages/team/page_helpers.ts`
  - added `resolveTeamPageNotice()` so the page can classify runtime control summaries separately
    from actual warnings.
- `web/src/pages/team_page.tsx`
  - runtime summaries such as `Team runtime updated (...)` and `Team runtime stopped (...)` now use
    a plain status notice with a dismiss button instead of the yellow warning `Alert`;
  - warnings like `Unable to initialize shared team thread.` still use the existing warning alert.
- `web/src/pages/team/page_helpers.test.ts`
  - added regression coverage for the new runtime-vs-warning notice classification.

## Why this fixes the issue

Before this change:

- successful runtime actions produced a yellow alert with warning iconography;
- users could reasonably read `Team runtime updated (started=3)` as an error banner.

After this change:

- successful runtime summaries look like status feedback, not failure feedback;
- only actual warning conditions keep the warning affordance.

## Validation

- `cd web && npx vitest run src/pages/team/page_helpers.test.ts`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team/page_helpers.ts src/pages/team/page_helpers.test.ts`
- `cd web && npm run build`

## Chrome DevTools MCP

Baseline on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` before
this edit:

- starting a Team showed `Team runtime update` in a warning-style alert surface;
- the body text was a success summary like `Team runtime updated (started=3)`.

Post-edit regression on the deployed domain is pending until this frontend change is rolled out.
