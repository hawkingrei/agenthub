# User Docs Deployment and Advanced Expansion

## Summary

Expand `userdocs` information architecture with deployment and advanced-usage
tracks, and strengthen troubleshooting guidance for connection-state recovery.

## Background

The user docs already covered core day-to-day single-agent workflows, but
deployment, automation, and multi-agent advanced usage paths were under-documented.
Operators and advanced users lacked a clear path for production rollout and
integration workflows.

## Scope

- `userdocs/sidebars.js`
- `userdocs/docs/intro.md`
- `userdocs/docs/operations/troubleshooting.md`
- `userdocs/docs/deployment/overview-and-topology.md`
- `userdocs/docs/deployment/production-checklist.md`
- `userdocs/docs/deployment/vercel-static-userdocs.md`
- `userdocs/docs/advanced/team-workbench.md`
- `userdocs/docs/advanced/openapi-and-automation.md`
- `userdocs/docs/advanced/connection-status-and-recovery.md`
- `docs/todo.md`

## Key Decisions

1. Add dedicated `Deployment` category to separate rollout runbooks from core
   usage steps.
2. Add `Advanced Usage` category for Team Workbench, OpenAPI integration, and
   connection-status recovery.
3. Keep all docs action-oriented, using concrete UI labels and endpoint paths.
4. Cross-link troubleshooting with connection-state docs to reduce debugging
   ambiguity between transport state and user-visible errors.

## Validation

```bash
cd userdocs
npm run build
```

Expected outcomes:

- Docusaurus build succeeds without broken links.
- New categories and pages appear in sidebar navigation.
- Existing core docs remain reachable and coherent.

## Follow-ups

- Add screenshots/GIFs for Team Workbench and connection badge states.
- Add docs lint/link check in CI for `userdocs` markdown quality.
