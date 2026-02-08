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
});
