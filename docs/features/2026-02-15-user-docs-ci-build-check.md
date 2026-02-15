# User Docs CI Build Check

## Summary

Add a dedicated GitHub Actions workflow to validate that `user-docs`
(Docusaurus site) can install dependencies and build on both pull requests and
pushes to `main`.

## Background

The user documentation site was added under `user-docs/`, but without CI
verification regressions in docs dependencies or config could go undetected
until manual checks.

## Scope

- `.github/workflows/user-docs.yml`
- `docs/todo.md`

## Key Decisions

1. Create an isolated `User Docs` workflow instead of piggybacking on existing
   `web` workflow to keep ownership and failure scope clear.
2. Use Node 20 to match existing frontend workflow runtime baseline.
3. Run `npm install` then `npm run build` in `user-docs/` as the minimum
   correctness gate for documentation publishing readiness.

## Validation

```bash
cd user-docs
npm install
npm run build
```

Expected outcomes:

- Dependency installation completes without lock/config errors.
- Docusaurus build succeeds and outputs static assets under
  `user-docs/build/`.

## Follow-ups

- Define and document deployment target for publishing generated docs.
