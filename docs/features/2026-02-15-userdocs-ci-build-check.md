# User Docs CI Build Check

## Summary

Add a dedicated GitHub Actions workflow to validate that `userdocs`
(Docusaurus site) can install dependencies and build on both pull requests and
pushes to `main`.

## Background

The user documentation site was added under `userdocs/`, but without CI
verification regressions in docs dependencies or config could go undetected
until manual checks.

## Scope

- `.github/workflows/userdocs.yml`
- `docs/todo.md`

## Key Decisions

1. Create an isolated `User Docs` workflow instead of piggybacking on existing
   `web` workflow to keep ownership and failure scope clear.
2. Use Node 20 to match existing frontend workflow runtime baseline.
3. Run `npm install` then `npm run build` in `userdocs/` as the minimum
   correctness gate for static documentation generation readiness.
4. Add path filters so this workflow runs only when `userdocs` or related docs
   metadata/workflow files change.
5. Add `workflow_dispatch` for manual rerun when validating doc-only updates.

## Validation

```bash
cd userdocs
npm install
npm run build
```

Expected outcomes:

- Dependency installation completes without lock/config errors.
- Docusaurus build succeeds and outputs static assets under
  `userdocs/build/`.

## Follow-ups

- Add `package-lock.json` for reproducible installs and migrate workflow install
  step from `npm install` to `npm ci`.
