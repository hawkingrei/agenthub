# Markdown Rendering Upgrade

## Background

ACP conversation messages were rendered with a regex-based formatter. With streaming chunks, unmatched backticks and emphasis markers could be paired across lines, producing broken HTML output and "encoding-like" artifacts.

## Scope

- Replace the custom formatter with `markdown-it`.
- Enable raw HTML while keeping link sanitization for safe output.
- Use default markdown soft breaks and add mobile-friendly spacing rules for rendered content.
- Add syntax highlighting for fenced code blocks via `highlight.js`.
- Extend markdown rendering tests for links and line breaks.
- Tune ACP markdown typography toward a prose-style layout (rhythm, headings, lists, code, and tables), including restoring list markers.

## Key Decisions

- Use `markdown-it` with `html: true`, `linkify: true`, and `typographer: true` for richer output while retaining link sanitization.
- Centralize link validation to allow http/https/mailto/tel and relative URLs only.
- Keep a single renderer instance to avoid repeated initialization overhead.
- Use `highlight.js` with a GitHub Dark theme so highlighted code remains readable on mobile and matches existing dark code blocks.
- Allow raw HTML from ACP output; if untrusted inputs are introduced, add a sanitization layer (for example DOMPurify) before rendering.

## Validation

```bash
cd web
npm test
```
