# Userdocs Build Root Link Fix

## Summary

Fix `userdocs` Docusaurus build failure caused by broken `/` links and migrate
deprecated markdown-link config to the current `markdown.hooks` location.

## Background

CI `npm run build` failed because many pages linked to `/` through shared
navbar/footer, but no document route was mounted at root. Build logs also
reported `onBrokenMarkdownLinks` deprecation warnings.

## Scope

- `userdocs/docs/intro.md`
- `userdocs/docusaurus.config.js`
- `docs/todo.md`

## Key Decisions

1. Set `intro` doc frontmatter `slug: /` so root path exists and shared `/`
   links remain valid.
2. Replace deprecated `siteConfig.onBrokenMarkdownLinks` with
   `siteConfig.markdown.hooks.onBrokenMarkdownLinks`.
3. Keep `onBrokenLinks: 'throw'` to preserve strict link-quality gating in CI.

## Validation

```bash
cd userdocs
npm install
npm run build
```

Expected outcomes:

- No broken-link errors for `/`
- No deprecation warning for `onBrokenMarkdownLinks`
- Build completes with generated output under `userdocs/build/`
