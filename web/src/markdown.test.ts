import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("removes unsafe javascript links", () => {
    const html = renderMarkdown("[click](javascript:alert(1))");
    expect(html).toContain("click");
    expect(html).not.toContain("<a ");
  });

  it("keeps safe http links", () => {
    const html = renderMarkdown("[site](https://example.com)");
    expect(html).toContain('href="https://example.com"');
    expect(html).toContain('target="_blank"');
    expect(html).toContain('rel="noopener noreferrer"');
  });

  it("blocks encoded javascript schemes", () => {
    const html = renderMarkdown("[click](java&#x73;cript:alert(1))");
    expect(html).toContain("click");
    expect(html).not.toContain("<a ");
  });

  it("keeps relative and hash links", () => {
    const relative = renderMarkdown("[rel](../docs/readme.md)");
    expect(relative).toContain('href="../docs/readme.md"');
    const hash = renderMarkdown("[section](#intro)");
    expect(hash).toContain('href="#intro"');
  });

  it("does not double-escape query strings", () => {
    const html = renderMarkdown("[search](/docs?q=one&two=3)");
    expect(html).toContain('href="/docs?q=one&amp;two=3"');
    expect(html).not.toContain("&amp;amp;");
  });

  it("auto-links whitelisted github pull URLs", () => {
    const html = renderMarkdown(
      "See https://github.com/hawkingrei/agenthub/pull/1233 for details."
    );
    expect(html).toContain(
      'href="https://github.com/hawkingrei/agenthub/pull/1233"'
    );
  });

  it("does not auto-link non-whitelisted bare URLs", () => {
    const html = renderMarkdown("See https://example.com/docs for details.");
    expect(html).toContain("https://example.com/docs");
    expect(html).not.toContain('href="https://example.com/docs"');
  });

  it("does not auto-link URLs inside inline or fenced code", () => {
    const html = renderMarkdown(
      "`https://github.com/hawkingrei/agenthub/pull/1`\n\n```txt\nhttps://github.com/hawkingrei/agenthub/pull/2\n```"
    );
    expect(html).not.toContain('href="https://github.com/hawkingrei/agenthub/pull/1"');
    expect(html).not.toContain('href="https://github.com/hawkingrei/agenthub/pull/2"');
  });

  it("does not auto-link inside unmatched shorter fenced close markers", () => {
    const html = renderMarkdown(
      "````txt\ncode line\n```\nhttps://github.com/hawkingrei/agenthub/pull/2\n````"
    );
    expect(html).not.toContain('href="https://github.com/hawkingrei/agenthub/pull/2"');
  });

  it("supports whitelisted URLs with parentheses in query parts", () => {
    const html = renderMarkdown(
      "See https://github.com/hawkingrei/agenthub/pull/1233?note=(alpha) for details."
    );
    expect(html).toContain(
      'href="https://github.com/hawkingrei/agenthub/pull/1233?note=(alpha)"'
    );
  });

  it("does not auto-link URLs inside multi-backtick inline code", () => {
    const html = renderMarkdown(
      "``https://github.com/hawkingrei/agenthub/pull/7`` and https://github.com/hawkingrei/agenthub/pull/8"
    );
    expect(html).not.toContain('href="https://github.com/hawkingrei/agenthub/pull/7"');
    expect(html).toContain('href="https://github.com/hawkingrei/agenthub/pull/8"');
  });

  it("skips whitelist autolink preprocessing for very large markdown inputs", () => {
    const prefix = "x".repeat(130_000);
    const html = renderMarkdown(
      `${prefix}\nhttps://github.com/hawkingrei/agenthub/pull/1233`
    );
    expect(html).toContain("https://github.com/hawkingrei/agenthub/pull/1233");
    expect(html).not.toContain(
      'href="https://github.com/hawkingrei/agenthub/pull/1233"'
    );
  });

  it("keeps soft line breaks without hard <br>", () => {
    const html = renderMarkdown("line one\nline two");
    expect(html).toContain("line one");
    expect(html).toContain("line two");
    expect(html).not.toContain("<br");
  });

  it("wraps fenced code blocks for highlighting", () => {
    const html = renderMarkdown("```ts\nconst value = 1\n```\n");
    expect(html).toContain('class="md-code-block hljs"');
    expect(html).toContain('data-language="ts"');
    expect(html).toContain("<pre");
    expect(html).toContain("<code");
  });

  it("adds rich markdown classes to headings, lists, blockquotes, tables, and inline code", () => {
    const html = renderMarkdown(
      "## Heading\n\n> quote\n\n- item\n\n`code`\n\n| a |\n| - |\n| b |"
    );
    expect(html).toContain('class="md-heading md-h2"');
    expect(html).toContain('class="md-blockquote"');
    expect(html).toContain('class="md-list md-list-unordered"');
    expect(html).toContain('class="md-list-item"');
    expect(html).toContain('class="md-inline-code"');
    expect(html).toContain('<div class="md-table-wrap"><table class="md-table">');
    expect(html).toContain('<th class="md-table_th">a</th>');
    expect(html).toContain('<td class="md-table_td">b</td>');
  });

  it("escapes raw html blocks", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"><b>safe</b>');
    expect(html).toContain("&lt;img");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<b>safe</b>");
  });

  it("adds markdown code-block classes to both pre and nested code tags", () => {
    const html = renderMarkdown("```ts\nconst value = 1\n```\n");
    expect(html).toContain('class="md-code-block hljs"');
    expect(html).toContain('class="md-code-block_code');
  });
});
