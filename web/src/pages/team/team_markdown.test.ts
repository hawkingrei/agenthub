import { describe, expect, it } from "vitest";
import { renderTeamMarkdownCached } from "./team_markdown";

describe("team markdown rendering", () => {
  it("escapes raw html and drops unsafe links", () => {
    const rendered = renderTeamMarkdownCached(
      `<script>alert(1)</script>\n<img src=x onerror=alert(1)>\n[unsafe](javascript:alert(1))`
    );

    expect(rendered).not.toContain("<script>");
    expect(rendered).not.toContain("<img");
    expect(rendered).toContain("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(rendered).toContain("&lt;img src=x onerror=alert(1)&gt;");
    expect(rendered).not.toContain('<a href="javascript:alert(1)"');
  });
});
