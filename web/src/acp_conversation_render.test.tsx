import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { AcpConversation } from "./components/acp_conversation";
import { ConversationItem } from "./conversation";

function renderConversation(
  items: ConversationItem[],
  override?: Partial<React.ComponentProps<typeof AcpConversation>>
): string {
  return renderToStaticMarkup(
    <AcpConversation
      items={items}
      windowOffset={0}
      isFrozenView={false}
      shouldAutoCollapse={false}
      collapseCutoff={0}
      stickToBottom={true}
      pendingCount={0}
      avgHeight={40}
      onScroll={() => {}}
      containerRef={React.createRef<HTMLDivElement>()}
      ansi={(input) => `<span class="ansi-out">${input}</span>`}
      {...override}
    />
  );
}

describe("AcpConversation rendering", () => {
  it("renders live tool calls expanded and terminal output through ansi renderer", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-1",
        title: "Shell",
        status: "in_progress",
        content: "line1\\nline2",
        raw_input: { cmd: "ls" },
        terminal_output: "stdout\\nline",
      },
    ]);

    expect(html).toContain("Tool Call: Shell");
    expect(html).toContain("in_progress");
    expect(html).toContain("acp-tool-fold");
    expect(html).toContain("open");
    expect(html).toContain("line1");
    expect(html).toContain("line2");
    expect(html).toContain("cmd");
    expect(html).toContain("ansi-out");
    expect(html).toContain("stdout");
  });

  it("renders finished tool calls collapsed by default", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-2",
        title: "Read",
        status: "completed",
      },
    ]);

    expect(html).toContain("Tool Call: Read");
    expect(html).not.toMatch(/acp-tool-fold\" open/);
  });

  it("renders auto-collapsed summaries for thinking and plan", () => {
    const html = renderConversation(
      [
        {
          kind: "agent_thinking",
          text: "thinking details for summary",
          live: false,
          event_id: 1,
        },
        {
          kind: "agent_plan",
          text: "1. do this",
          live: false,
          event_id: 2,
        },
      ],
      {
        shouldAutoCollapse: true,
        collapseCutoff: 10,
      }
    );

    expect(html).toContain("Thinking:");
    expect(html).toContain("Plan:");
  });

  it("renders markdown bubbles and pending spacer", () => {
    const html = renderConversation(
      [
        {
          kind: "agent_message",
          text: "**bold** message",
          event_id: 1,
        },
        {
          kind: "user_message",
          text: "plain user text",
          event_id: 2,
        },
      ],
      {
        stickToBottom: false,
        pendingCount: 3,
        avgHeight: 40,
      }
    );

    expect(html).toContain("<strong>bold</strong>");
    expect(html).toContain("plain user text");
    expect(html).toContain("acp-conversation-spacer");
    expect(html).toContain("height:120px");
  });
});
