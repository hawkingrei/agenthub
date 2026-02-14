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

  it("keeps soft line breaks without hard <br>", () => {
    const html = renderMarkdown("line one\nline two");
    expect(html).toContain("line one");
    expect(html).toContain("line two");
    expect(html).not.toContain("<br");
  });

  it("wraps fenced code blocks for highlighting", () => {
    const html = renderMarkdown("```ts\nconst value = 1\n```\n");
    expect(html).toContain('class="hljs"');
    expect(html).toContain("<pre");
    expect(html).toContain("<code");
  });

  it("escapes raw html blocks", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"><b>safe</b>');
    expect(html).toContain("&lt;img");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<b>safe</b>");
  });
});
