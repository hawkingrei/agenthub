# Web Coverage and Review Follow-ups

## Summary

Address review feedback and improve PR coverage for the web changes around ACP
permission history jump behavior and runtime viewport synchronization.

## Background

The PR had two concrete issues:

1. `jumpToConversationToolCall` could report success before the target bubble
   was actually found in DOM.
2. Coverage checks were red because newly introduced branch-heavy logic in
   `app.tsx` and ACP debug/hook paths lacked executable tests.

## Scope

- `web/src/app.tsx`
- `web/src/components/acp_debug.tsx`
- `web/src/hooks/use_acp_conversation.ts`
- `web/src/styles.css`
- `tests/web_assets.rs`
- `web/src/app.permission_scope.test.ts`
- `web/src/acp_debug.test.tsx`
- `web/src/acp_debug.interaction.test.tsx`
- `web/src/app.runtime_effects.test.tsx`
- `web/src/hooks/use_acp_conversation.interaction.test.tsx`
- `docs/features/2026-02-16-acp-permission-history-bubble-copy.md`

## Key Decisions

1. Refactor viewport/layout synchronization into exported helper setup
   functions in `app.tsx` so behavior is testable without relying only on E2E.
2. Change tool-call jump semantics to return `false` when target node is not
   immediately found, so caller-side retry can remain authoritative.
3. Add named constants for timing/retry knobs to remove magic numbers and make
   future tuning explicit.
4. Add `jsdom` test environment for interaction-level coverage in Vitest.
5. Align docs with shipped behavior: permission history click navigates to
   conversation instead of inline expansion.

## Validation

```bash
cd web
npm run test
npm run build

cd ..
cargo test --test web_assets
```

Expected outcomes:

- Web unit tests include node + jsdom interaction coverage for new branches.
- Tool-call jump retry path remains deterministic for caller logic.
- CSS contract test still passes while reducing brittle block-level assertions.
- Web build and Rust web-assets test pass.
