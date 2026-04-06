// @vitest-environment jsdom
import React from "react";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
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

  it("normalizes skill blocks before markdown rendering", () => {
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
    expect(html).toContain("<strong>Skill</strong>");
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

  it("reuses cached markdown for equivalent normalized skill blocks", () => {
    const first = renderThreadMarkdownCached(
      [
        "<skill>",
        "<name>team-actor-mailbox</name>",
        "<path>skills/team/team-actor-mailbox.SKILL.md</path>",
        "</skill>",
      ].join("\n")
    );
    const afterFirst = getThreadMarkdownCacheStats();
    const second = renderThreadMarkdownCached(
      "<skill><name>team-actor-mailbox</name><path>skills/team/team-actor-mailbox.SKILL.md</path></skill>"
    );
    const afterSecond = getThreadMarkdownCacheStats();

    expect(first).toContain("<strong>Skill</strong>");
    expect(second).toContain("<strong>Skill</strong>");
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

  it("renders mounted rich text with markdown on first render", async () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root: Root = createRoot(container);

    try {
      await act(async () => {
        root.render(
          <ThreadRichText
            text={[
              "Markdown ready:",
              "",
              "- first item",
              "- `inline-code`",
            ].join("\n")}
          />
        );
      });

      expect(container.innerHTML).toContain("<p>Markdown ready:</p>");
      expect(container.innerHTML).toContain("<ul>");
      expect(container.innerHTML).toContain("<code>inline-code</code>");
      const rootNode = container.querySelector(".acp-text") as HTMLDivElement | null;
      expect(rootNode).not.toBeNull();
      expect(rootNode?.className).toContain("max-w-full");
      expect(rootNode?.className).toContain("[overflow-wrap:anywhere]");
      expect(rootNode?.className).toContain("[&_pre]:whitespace-pre-wrap");
      expect(rootNode?.className).toContain("[&_pre_code]:break-words");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });
});
