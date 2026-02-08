import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("removes unsafe javascript links", () => {
    const html = renderMarkdown("[click](javascript:alert(1))");
    expect(html).toContain("click");
    expect(html).not.toContain("javascript:");
    expect(html).not.toContain("<a ");
  });

  it("keeps safe http links", () => {
    const html = renderMarkdown("[site](https://example.com)");
    expect(html).toContain('href="https://example.com"');
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
});
