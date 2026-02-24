# Docusaurus User Docs Site

## Summary

Add a dedicated Docusaurus-based user documentation website under `userdocs/`
to generate end-user guides for the core AgentHub workflow.

## Background

Existing repository docs are mostly engineering-facing (`docs/features`,
architecture notes, and TODO tracking). Users need a separate documentation
experience focused on operating AgentHub: setup, login, task execution, output
inspection, notifications, and troubleshooting.

## Scope

- `.gitignore`
- `README.md`
- `userdocs/package.json`
- `userdocs/docusaurus.config.js`
- `userdocs/sidebars.js`
- `userdocs/src/css/custom.css`
- `userdocs/README.md`
- `userdocs/docs/intro.md`
- `userdocs/docs/getting-started/installation.md`
- `userdocs/docs/getting-started/login.md`
- `userdocs/docs/core/create-agent.md`
- `userdocs/docs/core/run-and-interact.md`
- `userdocs/docs/core/view-output.md`
- `userdocs/docs/operations/notifications.md`
- `userdocs/docs/operations/troubleshooting.md`
- `docs/todo.md`

## Key Decisions

1. Keep user docs in a standalone `userdocs/` site to avoid mixing end-user
   pages with engineering/internal docs in `docs/`.
2. Use Docusaurus classic preset with `docs` only (`routeBasePath: /`) and no
   blog to keep information architecture simple.
3. Structure content by user journey:
   getting started -> core workflow -> operations.
4. Keep all content English and action-oriented, with concrete steps users can
   execute directly.
5. Add a repository-level entry point in `README.md` so contributors can preview
   docs locally with standard `npm` scripts.

## Validation

```bash
cd userdocs
npm install
npm run build
npm run start
```

Expected outcomes:

- Build completes and writes output to `userdocs/build/`
- Sidebar navigation includes all user-guide pages
- Root route serves `intro.md` as the landing page

## Follow-ups

- Add `README` guidance for optional external hosting providers if needed.
