# README and Docs Structure Refresh

## Summary

Refresh repository documentation entry points so contributors and users can find
setup, development, and docs workflows faster with fewer context switches.

## Background

The root `README.md` had useful information but lacked a clear documentation
map and consistent onboarding flow across runtime, development, and user docs.
`docs/` also lacked an index page describing internal documentation policy and
how `docs/` and `userdocs/` should be maintained together.

## Scope

- `README.md`
- `docs/README.md`
- `userdocs/README.md`
- `docs/todo.md`

## Key Decisions

1. Reorganize root README around practical user journeys: capabilities,
   quick start, configuration, common commands, docs map, and repository layout.
2. Add `docs/README.md` as the internal documentation index and maintenance
   checklist for contributors.
3. Clarify `userdocs/` as a static-site-generator workflow and include
   explicit Vercel static hosting settings.
4. Keep command examples aligned with existing scripts/targets (`cargo`, `web`,
   `make proto-check`) to reduce command drift.

## Validation

```bash
# verify markdown changes are tracked as expected
git -c core.fsmonitor=false status -sb

# verify user docs can still produce static build output
cd userdocs
npm run build
```

Expected outcomes:

- Root README provides a clear docs map and standard development entry points.
- `docs/README.md` defines internal documentation workflow.
- `userdocs/README.md` is directly usable for static deployment setup.

## Follow-ups

- Verify README command snippets remain aligned with future script changes.
- Consider adding markdown lint/link check workflow for root/docs/userdocs docs.
