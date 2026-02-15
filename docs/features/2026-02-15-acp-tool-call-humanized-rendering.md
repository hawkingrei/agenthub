# ACP Tool Call Humanized Rendering

## Background

Tool call sections in ACP conversation were showing `raw_input` / `raw_output`
as direct JSON blocks by default. This made frequent calls (search/open/click/shell)
hard to scan, especially on mobile and tablet layouts.

## Scope

- Improve tool call Input/Output rendering in ACP conversation.
- Keep existing fold behavior and terminal ANSI rendering unchanged.
- Avoid JSON-first display while preserving nested payload visibility.

## Key Decisions

- Introduce payload normalization for tool call Input/Output:
  - empty payloads are hidden,
  - JSON-like strings are parsed when safe,
  - plain text stays as text blocks.
- Render structured payloads as key-value views with nested fold sections
  instead of showing raw JSON directly.
- Add payload preview summarization using key signals (`query`, `cmd`, `path`, `url`, etc.)
  so section summaries are meaningful in compact mode.
- Detect unified diff payloads and render line-level visual classes
  (`meta` / `hunk` / `add` / `remove`) for quick scan of change intent.
- Detect ASCII-like multiline blocks and preserve fixed-width alignment
  (`white-space: pre`) to avoid shape distortion.
- Humanize tool call status labels (`in_progress` -> `In Progress`, etc.)
  for readability consistency with conversation UI.
- Add responsive CSS rules for payload rows:
  - desktop/tablet uses two-column key/value layout,
  - narrow mobile stacks key above value.

## Validation

```bash
cd web
npm run test -- src/acp_conversation_render.test.tsx src/acp_conversation.test.ts
```

- Expect tool call rendering tests to pass with:
  - structured payload assertions,
  - diff visualization assertions,
  - ASCII alignment class assertions,
  - status label rendering assertions,
  - existing fold and ANSI safety behavior unchanged.

## Follow-up

- Validate with real ACP sessions from Codex/Gemini/Kimi to ensure payload
  summarization quality and nested payload readability on long runs.
