# ACP Content Terminal Tone

## Summary

- Switched tool-call `Content` folds to a dedicated terminal-style presentation instead of the
  generic payload-document styling.
- Kept `Input`, `Output`, and `Detailed` payload cards unchanged so structured payloads still read
  like schema data instead of terminal logs.
- Added focused ACP conversation tests for plain-text and markdown content folds.

## Why

The `Content` fold often carries shell-like progress text, tool output, or semi-structured notes.
After the Notion-style UI refresh it still reused the light payload card styling, which clashed with
the adjacent terminal sections. A dedicated terminal tone makes the section visually coherent without
changing the semantics of other payload cards.

## Validation

- `cd web && npx vitest run src/acp_conversation.interaction.test.tsx --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/components/acp_conversation.tsx src/acp_conversation.interaction.test.tsx`
