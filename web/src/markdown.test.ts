import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("renders safe links", () => {
    const html = renderMarkdown("[link](https://example.com)");
    expect(html).toContain("href=\"https://example.com\"");
    expect(html).toContain(">link</a>");
  });

  it("drops javascript links", () => {
    const html = renderMarkdown("[x](javascript:alert(1))");
    expect(html).not.toContain("href=");
    expect(html).toContain("x");
  });

  it("drops encoded javascript links", () => {
    const html = renderMarkdown("[x](jav&#97;script:alert(1))");
    expect(html).not.toContain("href=");
  });

  it("keeps query strings without double escaping", () => {
    const html = renderMarkdown("[x](/path?a=1&b=2)");
    expect(html).toContain("href=\"/path?a=1&amp;b=2\"");
    expect(html).not.toContain("&amp;amp;");
  });

  it("allows relative links", () => {
    const html = renderMarkdown("[x](/path)");
    expect(html).toContain("href=\"/path\"");
  });
});
