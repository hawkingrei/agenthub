# Docusaurus User Docs Site

## Summary

Add a dedicated Docusaurus-based user documentation website under `user-docs/`
and publish end-user guides for the core AgentHub workflow.

## Background

Existing repository docs are mostly engineering-facing (`docs/features`,
architecture notes, and TODO tracking). Users need a separate documentation
experience focused on operating AgentHub: setup, login, task execution, output
inspection, notifications, and troubleshooting.

## Scope

- `.gitignore`
- `README.md`
- `user-docs/package.json`
- `user-docs/docusaurus.config.js`
- `user-docs/sidebars.js`
- `user-docs/src/css/custom.css`
- `user-docs/README.md`
- `user-docs/docs/intro.md`
- `user-docs/docs/getting-started/installation.md`
- `user-docs/docs/getting-started/login.md`
- `user-docs/docs/core/create-agent.md`
- `user-docs/docs/core/run-and-interact.md`
- `user-docs/docs/core/view-output.md`
- `user-docs/docs/operations/notifications.md`
- `user-docs/docs/operations/troubleshooting.md`
- `docs/todo.md`

## Key Decisions

1. Keep user docs in a standalone `user-docs/` site to avoid mixing end-user
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
cd user-docs
npm install
npm run build
npm run start
```

Expected outcomes:

- Build completes and writes output to `user-docs/build/`
- Sidebar navigation includes all user-guide pages
- Root route serves `intro.md` as the landing page

## Follow-ups

- Add CI workflow to verify `user-docs` install/build on pull requests.
- Define deployment target for publishing the static docs site.
