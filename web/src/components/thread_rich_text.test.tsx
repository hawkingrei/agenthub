import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it } from "vitest";
import {
  ThreadRichText,
  getThreadMarkdownCacheStats,
  preloadThreadMarkdownAssets,
  renderThreadMarkdownCached,
  resetThreadMarkdownCache,
} from "./thread_rich_text";

describe("thread_rich_text", () => {
  beforeEach(() => {
    resetThreadMarkdownCache();
  });

  it("normalizes skill blocks before fallback rich text rendering", () => {
    const html = renderToStaticMarkup(
      <ThreadRichText
        text={[
          "Review these references:",
          "",
          "<skill>",
          "<name>team-actor-mailbox</name>",
          "<path>skills/team/team-actor-mailbox.SKILL.md</path>",
          "</skill>",
        ].join("\n")}
      />
    );

    expect(html).toContain("Review these references:");
    expect(html).toContain("**Skill**");
    expect(html).toContain("team-actor-mailbox");
    expect(html).toContain("skills/team/team-actor-mailbox.SKILL.md");
    expect(html).not.toContain("&lt;skill&gt;");
  });

  it("records cache hits after markdown assets are preloaded", async () => {
    await preloadThreadMarkdownAssets();

    const first = renderThreadMarkdownCached("## Heading\n\n- item");
    const afterFirst = getThreadMarkdownCacheStats();
    const second = renderThreadMarkdownCached("## Heading\n\n- item");
    const afterSecond = getThreadMarkdownCacheStats();

    expect(first).toContain("<h2>Heading</h2>");
    expect(second).toContain("<h2>Heading</h2>");
    expect(afterFirst.markdownMisses).toBe(1);
    expect(afterFirst.markdownHits).toBe(0);
    expect(afterSecond.markdownMisses).toBe(1);
    expect(afterSecond.markdownHits).toBe(1);
  });

  it("bypasses the cache for oversized markdown entries", async () => {
    await preloadThreadMarkdownAssets();

    const oversized = `${"a".repeat(120_001)}\n\n\`\`\`ts\nconst value = 1;\n\`\`\``;
    renderThreadMarkdownCached(oversized);
    renderThreadMarkdownCached(oversized);
    const stats = getThreadMarkdownCacheStats();

    expect(stats.markdownMisses).toBe(2);
    expect(stats.markdownHits).toBe(0);
  });
});
