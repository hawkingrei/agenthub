import { describe, expect, it } from "vitest";
import { escapeTeamHtml, isTeamImeComposing } from "./team_text_helpers";

describe("team_text_helpers", () => {
  it("escapes html-sensitive characters", () => {
    expect(escapeTeamHtml(`<team attr="1">'&"</team>`)).toBe(
      "&lt;team attr=&quot;1&quot;&gt;&#39;&amp;&quot;&lt;/team&gt;"
    );
  });

  it("treats ime composition and keyCode 229 as composing", () => {
    expect(isTeamImeComposing(true, false, undefined)).toBe(true);
    expect(isTeamImeComposing(false, true, undefined)).toBe(true);
    expect(isTeamImeComposing(false, false, 229)).toBe(true);
    expect(isTeamImeComposing(false, false, 13)).toBe(false);
  });
});
