# Userdocs Content Expansion

## Summary

Expand the `userdocs` site with more user-facing operational content, including
first-run walkthrough, configuration basics, workdir/worktree strategy, session
lifecycle, review workflow, prompt patterns, checklist, security guidance, and
FAQ.

## Background

The initial userdocs version covered basic setup and operations. Users requested
broader guidance for day-to-day usage and more concrete execution patterns.

## Scope

- `userdocs/sidebars.js`
- `userdocs/docs/intro.md`
- `userdocs/docs/getting-started/first-task-walkthrough.md`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/getting-started/installation.md`
- `userdocs/docs/getting-started/login.md`
- `userdocs/docs/core/workdir-worktree-strategy.md`
- `userdocs/docs/core/session-lifecycle.md`
- `userdocs/docs/core/review-and-apply-changes.md`
- `userdocs/docs/prompting/task-instruction-patterns.md`
- `userdocs/docs/core/run-and-interact.md`
- `userdocs/docs/operations/daily-operations-checklist.md`
- `userdocs/docs/operations/security-and-path-safety.md`
- `userdocs/docs/operations/notifications.md`
- `userdocs/docs/operations/troubleshooting.md`
- `userdocs/docs/operations/faq.md`
- `docs/todo.md`

## Key Decisions

1. Keep the docs user-journey oriented and add missing middle layers:
   onboarding -> execution strategy -> lifecycle -> troubleshooting.
2. Add a dedicated prompting section to standardize higher-quality task
   instructions.
3. Add explicit user-facing review and safety pages so operational risk is
   addressed without requiring architecture context.
4. Keep guidance practical, with examples and short checklists instead of
   architecture-heavy descriptions.
5. Preserve existing page URLs and only append new pages/categories for safer
   incremental adoption.

## Validation

```bash
cd userdocs
npm install
npm run build
npm run start
```

Expected outcomes:

- New sections appear in sidebar (`Configuration Basics`, `Review and Apply
  Changes`, `Daily Operations Checklist`, `Security and Path Safety`)
- No broken links in docs build output
- Intro reading paths map to actual pages

## Follow-ups

- Add screenshots/GIFs for key flows (create agent, output view, reconnect).
- Add role-based quick paths (individual developer vs team lead).
- Add short copy-paste templates for common task types (bugfix/refactor/docs).
