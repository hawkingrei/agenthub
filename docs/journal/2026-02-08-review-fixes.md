# Review Follow-up Fixes

## Background

Review feedback flagged two areas: missing markdown sanitization coverage and incomplete keyboard behavior in the Agents list.

## Scope

- Add `renderMarkdown` tests for encoded `javascript:` schemes, relative links, hash links, and query strings.
- Allow Enter/Space to activate focusable agent rows to match button-like behavior.

## Key Decisions

- Keep rendering logic unchanged; extend tests to prevent regressions.
- Use minimal keyboard handling changes to avoid larger layout or style impact.

## Validation

```bash
cd web
npm test
```
