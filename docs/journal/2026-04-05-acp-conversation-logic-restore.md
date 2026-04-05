## Summary

Restored `web/src/components/acp_conversation.tsx` to the `origin/main` logic after the first-stage bubble split caused Agent ACP views to regress to an empty state in practice.

## What changed

- Reverted `web/src/components/acp_conversation.tsx` to the pre-split implementation from `origin/main`.
- Removed the temporary extracted bubble components under `web/src/components/bubbles/`.
- Added shared ACP plan summary logic so the `Plan` tab now shows live status when a plan exists:
  - `N active` when there are in-progress entries
  - `N pending` when there are queued entries but no active ones
  - `done` when every entry is completed

## Why

The visual direction can stay aligned with the current workbench styling, but ACP interaction and rendering behavior must remain consistent with the previously working implementation.

Given the current branch delta, the safest fix was to restore the known-good ACP conversation logic first, then re-approach component extraction later with narrower, behavior-preserving steps.

## Validation

- `cd web && npm run test -- src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx src/components/thread_rich_text.test.tsx`
- `cd web && npm run lint -- src/components/acp_conversation.tsx`
- `cd web && npm run build`
- `cd web && npm run test -- src/acp_panel.test.tsx src/input_dock_render.test.tsx src/output_cache.test.ts`
- `cd web && npm run lint`
- `make build-web`

## Follow-up

- If ACP bubble extraction is resumed, split only one bubble type at a time and keep behavior parity checks around:
  - conversation item visibility
  - tool fold open/collapse behavior
  - markdown rendering
  - request-user-input cards
