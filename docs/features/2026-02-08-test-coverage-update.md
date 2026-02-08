# Test Coverage Update

## Background

Recent refactors introduced new storage and agent status helpers without direct unit coverage.

## Scope

- Add unit tests for output cache storage load/save behavior.
- Add unit tests for agent status helper functions.
- Add unit tests for event cursor ordering and conversation freeze behavior.

## Key Decisions

- Keep tests in the existing `vitest` node environment with lightweight in-memory storage stubs.
- Avoid React component tests to prevent introducing new dependencies.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Add React component tests once a testing library is adopted.
