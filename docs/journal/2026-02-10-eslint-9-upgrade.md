# ESLint 9 Upgrade

## Background
The frontend dependency tree produced multiple deprecation warnings tied to ESLint 8 and legacy config packages. Upgrading to ESLint 9 removes those legacy dependencies and aligns the tooling with current defaults while staying compatible with TypeScript ESLint peer requirements.

## Scope
- Upgrade ESLint and TypeScript ESLint packages.
- Migrate to flat config (`web/eslint.config.js`).
- Update linting documentation to reflect the new baseline.

## Key Decisions
- Use ESLint 9 with flat config and explicit browser/node globals.
- Keep lint rules equivalent to the previous recommended rule sets.

## Validation
```bash
cd web
npm install
npm run lint
```
