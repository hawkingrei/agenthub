# ACP Conversation Runtime Metrics and Segmented Rendering

## Background

ACP conversation tool-call payloads could still create heavy render cost when
payloads or terminal streams were large. Debug view also lacked runtime
observability for conversation rendering behavior, which made regression checks
hard during long sessions.

## Scope

- Add lazy rendering hooks for tool-call sub-sections.
- Add segmented rendering for large payload/object/list/text/diff content.
- Keep markdown/diff/ascii rendering semantics for human readability.
- Expose runtime rendering metrics in ACP Debug.

## Key Decisions

- Keep tool-call summary behavior unchanged, but make Input/Output/Content/Terminal
  bodies lazy-mount via fold activation.
- Treat JSON-like payload strings as `json_text` first; parse only when the
  section body is actually rendered.
- Add progressive "Show more" chunking for:
  - plain text payload blocks,
  - unified diff views,
  - terminal output rendering,
  - array/object payload collections.
- Add markdown fallback guard for very large markdown payloads:
  - switch to plain text segmented view to protect UI responsiveness.
- Extend ACP Debug with `Runtime` tab:
  - conversation total/source/rendered counts,
  - virtualization/stick-to-bottom state,
  - cache hit/miss counters (markdown + ansi),
  - payload JSON parse success/failure counters.

## Validation

```bash
cd web
npm run test -- src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_debug.test.tsx src/acp_panel.test.tsx src/output_body.test.tsx src/hooks/use_acp_conversation.test.ts
npm run build
```

- Expect render tests to cover:
  - structured payload rendering,
  - segmented "Show more" behavior for long content,
  - diff/ascii/markdown rendering paths.
- Expect debug tests to include runtime tab contract.

## Follow-up

- Run a real long-session ACP replay (search/explore/shell mixed) and confirm:
  - no noticeable fold-open lag for large payloads,
  - runtime metrics align with observed rendering behavior.
