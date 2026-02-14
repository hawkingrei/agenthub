# ACP Mobile Tab Compact Sizing

## Summary

Reduce the visual footprint of ACP tab controls on phones so `Conversation` /
`Debug` tab cards no longer look oversized in narrow viewports.

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
  - shrink ACP header gap and action spacing,
  - reduce top tab chip padding/font size,
  - reduce badge size,
  - reduce interrupt button size,
  - reduce debug tab chip size.
- For `@media (max-width: 420px)`:
  - apply an additional compact pass to tabs and interrupt button.
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
