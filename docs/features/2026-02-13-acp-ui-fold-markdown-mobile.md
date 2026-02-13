# ACP UI Fold, Markdown, and Mobile Alignment Fixes

## Background

Three regressions were observed in ACP conversation UI:

- Some tool call blocks did not auto-collapse reliably when live execution ended.
- Markdown rendering looked inconsistent, especially for list and code typography.
- Mobile layout alignment was unstable around safe-area and ACP header actions.

## Scope

- Fix tool call fold state transitions in conversation rendering.
- Improve markdown typography and prevent admin list styles from leaking into ACP markdown.
- Improve mobile layout alignment for ACP controls and panel safe-area behavior.

## Key Decisions

- Add a tool call fold state transition helper to ensure:
  - live tool calls stay open,
  - live-to-finished transitions auto-collapse,
  - manual toggles for finished calls remain respected.
- Use stable conversation keys (tool call ID / event cursor) to avoid fold state drift during list updates.
- Scope admin list styles to `.admin .card` only, so markdown list items keep semantic layout.
- Add `100dvh` support and safe-area padding/offset handling for mobile viewport alignment.
- Stack ACP header controls on narrow screens to prevent action bar overflow.

## Validation

```bash
cd web
npm run test
```

- Expect all Vitest suites to pass, including:
  - `src/acp_conversation.test.ts`
  - `src/conversation.test.ts`

```bash
cargo test --test web_assets
```

- Expect CSS guard assertions to pass, including:
  - mobile `100dvh` support
  - admin list style scoping
  - mobile ACP header stacking rule
