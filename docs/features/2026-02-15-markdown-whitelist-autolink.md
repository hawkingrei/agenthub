# Markdown Whitelist Auto-Link

## Background

Conversation markdown often includes plain URLs. Rendering all bare URLs as links
is not desirable, but GitHub PR URLs should be clickable by default for review flow.

## Scope

- Add whitelist-based bare URL auto-linking in markdown rendering.
- Keep existing explicit markdown links (`[text](url)`) behavior unchanged.
- Preserve security behavior: do not relax href sanitization rules.

## Key Decisions

- Keep `markdown-it` `linkify` disabled.
- Add a pre-render transform that only converts whitelist bare URLs to markdown links.
- Initial whitelist rule:
  - `https://github.com/<owner>/<repo>/pull/<number>[...]`
- Skip auto-link conversion in:
  - fenced code blocks,
  - inline code spans,
  - already-linked contexts.
- Keep non-whitelist bare URLs as plain text.

## Validation

```bash
cd web
npm run test -- src/markdown.test.ts
```

- Expect tests to cover:
  - whitelisted GitHub PR auto-link success,
  - non-whitelist URL not auto-linked,
  - inline/fenced code URL not auto-linked.

## Follow-up

- Extend whitelist patterns if product needs additional trusted URL families
  (for example issues/commits/releases) after usage feedback.
