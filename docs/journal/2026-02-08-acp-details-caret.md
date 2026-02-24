# ACP Details Caret Indicator

## Background

Tool call and thinking detail sections rely on the browser's default disclosure marker, which is subtle and inconsistent across platforms.

## Scope

- Add explicit caret indicators to ACP `details` summaries for tool calls and thinking blocks.

## Key Decisions

- Use the same caret glyph for both tool and thinking sections to keep the UI consistent.
- Hide the native `summary` marker to avoid double indicators.

## Validation

```bash
Open the ACP conversation view and confirm tool calls and thinking sections show ▸ when collapsed and ▾ when expanded.
```
