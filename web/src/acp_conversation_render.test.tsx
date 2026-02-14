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
      runStatus={null}
      virtualTopSpacer={0}
      virtualBottomSpacer={0}
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

  it("sanitizes terminal html while keeping allowed ansi span tags", () => {
    const html = renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-unsafe",
          title: "Shell",
          status: "in_progress",
          terminal_output: "ignored",
        },
      ],
      {
        ansi: () =>
          '<span style="color:#e06c75">safe</span><img src=x onerror=alert(1)>',
      }
    );

    expect(html).toContain("<span style=\"color:#e06c75\">safe</span>");
    expect(html).toContain("&lt;img");
    expect(html).not.toContain("<img");
  });

  it("keeps nested ansi style spans and combines inherited styles", () => {
    const html = renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-nested-ansi",
          title: "Shell",
          status: "completed",
          terminal_output: "ignored",
        },
      ],
      {
        ansi: () =>
          '<span style="color:#123456">outer <span style="font-weight:700">inner</span> tail</span>',
      }
    );

    expect(html).toContain("outer");
    expect(html).toContain("inner");
    expect(html).toContain("tail");
    expect(html).toContain("style=\"color:#123456\"");
    expect(html).toContain("style=\"color:#123456;font-weight:700\"");
  });

  it("drops unsupported ansi styles and escapes non-whitelisted span tags", () => {
    const html = renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-style-filter",
          title: "Shell",
          status: "completed",
          terminal_output: "ignored",
        },
      ],
      {
        ansi: () =>
          '<span style="position:absolute;color:#111111">safe</span><span class="ansi-out">literal</span>',
      }
    );

    expect(html).toContain("<span style=\"color:#111111\">safe</span>");
    expect(html).not.toContain("position:absolute");
    expect(html).toContain("&lt;span class=&quot;ansi-out&quot;&gt;literal&lt;/span&gt;");
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
    expect(html).not.toMatch(/acp-tool-fold" open/);
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

  it("renders plan entries as a structured plan card", () => {
    const html = renderConversation([
      {
        kind: "agent_plan",
        text: "1. analyze\n2. implement",
        plan_entries: [
          { content: "analyze", status: "completed", priority: "high" },
          { content: "implement", status: "in_progress" },
        ],
        event_id: 2,
      },
    ]);

    expect(html).toContain("acp-plan-card");
    expect(html).toContain("2 completed");
    expect(html).toContain("analyze");
    expect(html).toContain("implement");
    expect(html).toContain("in_progress");
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
