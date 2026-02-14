# ACP Mobile Tab Compact Sizing

## Summary

Reduce the visual footprint of ACP tab controls on phones and make sizing
adaptive across different narrow viewport widths.

## Background

On mobile screens, ACP top controls (interrupt + conversation/debug tabs) and
debug sub-tabs had desktop-sized padding and typography, which made the tab
chips appear too large.

## Scope

- `web/src/styles.css`
- `tests/web_assets.rs`
- `docs/todo.md`

## Key Decisions

- Keep desktop/tabet styles unchanged; apply compact sizing only under mobile
  breakpoints.
- For `@media (max-width: 720px)`:
  - use `clamp(...)` for tab font sizes/paddings and interrupt button sizing,
  - center and equalize top tabs with flexible width (`flex: 1 1 0`),
  - keep badge sizing adaptive with viewport-scaled spacing,
  - allow debug tabs to wrap on narrower widths.
- Remove fixed 420px-only overrides and use fluid sizing instead.
- Update CSS guard test to assert compact mobile tab styles are present.

## Validation

```bash
cd web
npm run build
```

```bash
cargo test --test web_assets
```

## Follow-ups

- Verify real-device readability and tap target comfort on iOS Safari and
  Android Chrome.
