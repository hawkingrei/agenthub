# Web Mobile Viewport Alignment

## Summary

Improve AgentHub web shell viewport alignment on phone/tablet devices:

1. Drive app shell width/height from runtime viewport CSS variables.
2. Track mobile browser dynamic viewport changes (`visualViewport`) and sync
   `--agenthub-vh` / `--agenthub-vw`.
3. Anchor mobile agents drawer top position to measured workspace offset instead
   of a hardcoded pixel value.

## Background

On mobile browsers, `100vh` can drift from the real visible area when browser
chrome expands/collapses or when keyboard opens. This can cause clipped content
and drawer offset mismatch, especially on iPad/phone layouts.

## Scope

- `web/src/app.tsx`
- `web/src/styles.css`
- `web/index.html`
- `tests/web_assets.rs`
- `docs/todo.md`

## Key Decisions

1. Keep CSS fallback defaults (`100vh` / `100vw`) and override with runtime
   pixel values from `visualViewport` when available.
2. Use `ResizeObserver` + resize/orientation listeners to keep header/workspace
   anchor variables fresh without polling.
3. Preserve existing responsive structure and only replace hardcoded mobile
   drawer top with `--agenthub-workspace-top`.
4. Add `viewport-fit=cover` to support safe-area based layout on modern iOS.

## Validation

```bash
cd web
npm run test -- use_acp_conversation.test.ts acp_debug.test.tsx acp_panel.test.tsx output_body.test.tsx acp_debug_permissions.test.ts app.permission_scope.test.ts acp_conversation_render.test.tsx
npm run build

cd ..
cargo test --test web_assets
```

Expected outcomes:

- App shell fits visible viewport on mobile/tablet during resize/orientation.
- Agents drawer aligns with workspace top on mobile.
- No horizontal overflow introduced by viewport sizing changes.
- CSS guard test passes with updated viewport contract.

## Follow-ups

- Add Playwright viewport/keyboard E2E checks for iPhone/iPad profiles.
- Consider extracting viewport-sync logic into a dedicated hook.
