# Team Page Workbench Split

## Summary

- extracted the main Team workbench render tree from `web/src/pages/team_page.tsx` into `web/src/pages/team/team_workbench_content.tsx`
- kept `TeamPage` state, effects, and action wiring unchanged while moving shell-level panel selection into a dedicated component
- preserved the existing workspace shell convergence behavior and tests while reducing the deepest JSX nesting inside `TeamPage`

## Validation

```bash
make build-web
cd web && npm run test -- vite.config.test.ts src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx src/pages/team/team_page_header.test.tsx
```

## Follow-up

- `web/src/pages/team_page.tsx` is still much larger than the preferred file-boundary target, so the next safe splits should focus on derived workspace view-model state and Team management callbacks rather than additional shell JSX.
