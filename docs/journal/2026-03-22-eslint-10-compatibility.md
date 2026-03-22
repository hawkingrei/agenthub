## Summary

Adjusted the ESLint 10 dependency bump so the web toolchain remains installable and lintable under
the current React plugin ecosystem.

## Scope

- update `web/package.json` and `web/package-lock.json` for an ESLint 10-compatible dependency set
- narrow `web/eslint.config.js` to the supported lint surface
- fix new lint findings surfaced by the ESLint 10 / `@eslint/js` 10 rule set

## Notes

- `@typescript-eslint/eslint-plugin` and `@typescript-eslint/parser` were moved to `8.57.1`
  because that stable line advertises ESLint 10 support.
- `eslint-plugin-react` was removed from the active lint stack because its published stable release
  still advertises support only through ESLint 9.x.
- `eslint-plugin-react-hooks` was moved to a canary build that advertises ESLint 10 support so the
  project keeps hook validation without blocking the dependency upgrade.
- Four existing web files needed small code adjustments for newly enforced lint findings:
  - `web/public/sw.js`
  - `web/src/conversation.ts`
  - `web/src/pages/team/create_helpers.ts`
  - `web/src/storage/output_cache_storage.ts`

## Validation

- `npm install`
- `npm run lint`
- `npm run test`
- `npm ci`
- `npm run build`
