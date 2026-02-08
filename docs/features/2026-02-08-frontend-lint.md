# Frontend Linting

## Background

The frontend currently lacks a lint step. Adding ESLint provides consistent code-quality checks alongside tests.

## Scope

- Add ESLint configuration for TypeScript + React.
- Expose `npm run lint` in the web package.
- Provide `make lint` as the repo entry point.
- Run lint in CI for the web job.
- Resolve lint findings in hooks and regex handling.

## Key Decisions

- Use ESLint 8 with `.eslintrc.cjs` to keep configuration simple.
- Enable React and React Hooks recommended rules.

## Validation

```bash
cd web
npm run lint
```
