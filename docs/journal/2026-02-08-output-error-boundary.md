# Output Error Boundary And Empty State

## Background

We observed intermittent Output Body black screens after recent UI refactors. The issue was difficult to diagnose because runtime render failures produced a blank area with no visible status.

## Scope

- Add a dedicated error boundary around the Output body.
- Render a clear fallback message when Output fails to render.
- Add a lightweight empty state when no output is available.

## Key Decisions

- Keep the error boundary local to Output to avoid masking unrelated UI errors.
- Use a minimal empty state to avoid confusing it with the loading indicator.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Capture console errors when black screens occur to pinpoint the root cause.
- Consider logging error details in the backend audit stream.
