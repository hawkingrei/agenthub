# use_acp_conversation Coverage Hardening

## Summary

Increase branch and line coverage for `use_acp_conversation` by extracting
decision-heavy logic into pure helper functions and adding focused unit tests
for each helper.

## Background

The conversation hook had many conditional paths (virtualization, history
autoload, viewport updates, jump badge visibility, scroll restore) but low
direct unit coverage. Most logic lived inside hook effects/callbacks, making
branch-level verification expensive and brittle.

## Scope

- `web/src/hooks/use_acp_conversation.ts`
- `web/src/hooks/use_acp_conversation.test.ts`
- `docs/todo.md`

## Key Decisions

1. Keep hook behavior unchanged while extracting deterministic calculations.
2. Export only decision/helper functions that do not depend on React runtime.
3. Route existing hook branches through extracted helpers to preserve single
   behavior source.
4. Add targeted tests for:
   - tail key generation
   - history loading gate conditions
   - virtualization threshold decision
   - viewport clamping/update rules
   - auto-load eligibility
   - average height normalization
   - scroll restoration fallback
   - jump badge/jump button derivation

## Validation

```bash
cd web
npm run test -- src/hooks/use_acp_conversation.test.ts
npm run lint -- src/hooks/use_acp_conversation.ts src/hooks/use_acp_conversation.test.ts
npm run build
npm run test:coverage
```

## Follow-ups

- Confirm the extracted helper path keeps conversation scroll UX stable on long
  real-world sessions (desktop + mobile).
