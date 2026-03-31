import { beforeEach, describe, expect, it, vi } from "vitest";
import { layout, prepare } from "@chenglou/pretext";
import type { ConversationItem } from "../conversation";

vi.mock("@chenglou/pretext", () => ({
  prepare: vi.fn((text: string) => ({ text })),
  layout: vi.fn(
    (
      prepared: { text: string },
      maxWidth: number,
      lineHeight: number
    ) => {
      const width = Math.max(1, maxWidth);
      const estimatedCharsPerLine = Math.max(1, Math.floor(width / 8));
      const text = prepared.text ?? "";
      const lines = Math.max(
        1,
        text
          .split(/\n/)
          .reduce(
            (count, line) =>
              count + Math.max(1, Math.ceil(Math.max(1, line.length) / estimatedCharsPerLine)),
            0
          )
      );
      return {
        lineCount: lines,
        height: lines * lineHeight,
      };
    }
  ),
}));

import {
  buildConversationHeightEstimateModel,
  buildVirtualConversationSliceWithHeightModel,
  estimateConversationItemHeight,
  resetConversationHeightEstimateCaches,
} from "./conversation_height_estimate";

function makeMessage(text: string, eventId: number): ConversationItem {
  return {
    kind: "agent_message",
    text,
    event_id: eventId,
  };
}

describe("conversation_height_estimate", () => {
  beforeEach(() => {
    resetConversationHeightEstimateCaches();
  });

  it("estimates taller rows for longer markdown messages", () => {
    const shortHeight = estimateConversationItemHeight(
      makeMessage("short message", 1),
      720,
      48
    );
    const longHeight = estimateConversationItemHeight(
      makeMessage("long ".repeat(200), 2),
      720,
      48
    );
    expect(longHeight).toBeGreaterThan(shortHeight);
  });

  it("falls back for non-message items", () => {
    const height = estimateConversationItemHeight(
      {
        kind: "tool_call",
        id: "call-1",
        title: "Read",
        event_id: 1,
      },
      720,
      64
    );
    expect(height).toBe(64);
  });

  it("does not add extra chrome when pretext measurement is unavailable", () => {
    const prepareMock = vi.mocked(prepare);
    resetConversationHeightEstimateCaches();
    try {
      prepareMock.mockImplementationOnce(() => {
        throw new Error("measurement unavailable");
      });
      const height = estimateConversationItemHeight(
        makeMessage("plain text", 3),
        720,
        48
      );
      expect(height).toBe(48);
    } finally {
      prepareMock.mockReset();
      prepareMock.mockImplementation((text: string) => ({ text }) as never);
      resetConversationHeightEstimateCaches();
    }
  });

  it("produces larger total height when the viewport narrows", () => {
    const items = [
      makeMessage("word ".repeat(120), 1),
      makeMessage("another ".repeat(80), 2),
    ];
    const wide = buildConversationHeightEstimateModel(items, 720, 48);
    const narrow = buildConversationHeightEstimateModel(items, 320, 48);
    expect(narrow.totalHeight).toBeGreaterThan(wide.totalHeight);
  });

  it("builds variable-height slices with prefix-sum spacers", () => {
    const items = Array.from({ length: 200 }, (_, index) =>
      makeMessage(`message-${index} ${"x ".repeat(index % 15)}`, index + 1)
    );
    const model = buildConversationHeightEstimateModel(items, 640, 48);
    const slice = buildVirtualConversationSliceWithHeightModel(
      3200,
      480,
      model,
      8
    );
    expect(slice.start).toBeGreaterThan(0);
    expect(slice.end).toBeGreaterThan(slice.start);
    expect(slice.topSpacer).toBeGreaterThan(0);
    expect(slice.bottomSpacer).toBeGreaterThan(0);
    expect(slice.topSpacer + slice.bottomSpacer).toBeLessThan(model.totalHeight);
  });

  it("adds extra room for fenced code blocks", () => {
    const plain = estimateConversationItemHeight(
      makeMessage("explain the fix briefly", 4),
      720,
      48
    );
    const withCode = estimateConversationItemHeight(
      makeMessage(
        [
          "explain the fix briefly",
          "",
          "```ts",
          "const value = veryLongIdentifierName + anotherLongIdentifierName;",
          "console.log(value);",
          "```",
        ].join("\n"),
        5
      ),
      720,
      48
    );
    expect(withCode).toBeGreaterThan(plain);
  });

  it("does not make fenced code height depend on viewport width", () => {
    const codeOnly = makeMessage(
      [
        "```ts",
        "const value = veryLongIdentifierName + anotherLongIdentifierName;",
        "console.log(value);",
        "```",
      ].join("\n"),
      6
    );
    const wide = estimateConversationItemHeight(codeOnly, 720, 48);
    const narrow = estimateConversationItemHeight(codeOnly, 320, 48);
    expect(narrow).toBe(wide);
  });

  it("reuses measured heights when the fallback average changes", () => {
    estimateConversationItemHeight(
      makeMessage("cache me if you can", 8),
      720,
      48
    );
    const layoutMock = vi.mocked(layout);
    const firstPassCalls = layoutMock.mock.calls.length;

    estimateConversationItemHeight(
      makeMessage("cache me if you can", 9),
      720,
      96
    );

    expect(layoutMock.mock.calls.length).toBe(firstPassCalls);
  });

  it("adds structure compensation for markdown blockquotes", () => {
    const plain = estimateConversationItemHeight(
      makeMessage("Important follow-up", 10),
      720,
      48
    );
    const structured = estimateConversationItemHeight(
      makeMessage("> Important follow-up", 11),
      720,
      48
    );
    expect(structured).toBeGreaterThan(plain);
  });

  it("preserves inline hash and comparison characters in measured text", () => {
    estimateConversationItemHeight(
      makeMessage("Issue #123 checks whether a > b", 12),
      720,
      48
    );
    const prepareMock = vi.mocked(prepare);
    expect(
      prepareMock.mock.calls.some(([text]) =>
        String(text).includes("#123") && String(text).includes("a > b")
      )
    ).toBe(true);
  });

  it("measures markdown headings with heading-specific fonts", () => {
    estimateConversationItemHeight(
      makeMessage("# Release notes\n\n#### subsection", 13),
      720,
      48
    );
    const prepareMock = vi.mocked(prepare);
    expect(
      prepareMock.mock.calls.some(([, font]) =>
        String(font).includes('600 18px "Space Grotesk"')
      )
    ).toBe(true);
    expect(
      prepareMock.mock.calls.some(([, font]) =>
        String(font).includes('600 14px "Space Grotesk"')
      )
    ).toBe(true);
  });

  it("adds conservative height for markdown images", () => {
    const plain = estimateConversationItemHeight(
      makeMessage("status update", 16),
      720,
      48
    );
    const withImage = estimateConversationItemHeight(
      makeMessage("![diagram](https://example.com/diagram.png)", 17),
      720,
      48
    );
    expect(withImage).toBeGreaterThan(plain);
  });

  it("uses the ACP text font family when preparing rich text", () => {
    estimateConversationItemHeight(makeMessage("font check", 18), 720, 48);
    const prepareMock = vi.mocked(prepare);
    expect(
      prepareMock.mock.calls.some(([, font]) =>
        String(font).includes("Space Grotesk")
      )
    ).toBe(true);
  });
});
