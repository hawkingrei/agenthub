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
    expect(html).toContain("In Progress");
    expect(html).toContain("acp-tool-fold");
    expect(html).toContain("open");
    expect(html).toContain("Input");
    expect(html).toContain("cmd=ls");
    expect(html).not.toContain("&quot;cmd&quot;");
    expect(html).toContain("line1");
    expect(html).toContain("line2");
    expect(html).toContain("ansi-out");
    expect(html).toContain("stdout");
  });

  it("renders grouped tool calls with a shared fold and nested tool entries", () => {
    const html = renderConversation([
      {
        kind: "tool_call_group",
        event_id: 12,
        calls: [
          {
            kind: "tool_call",
            id: "call-1",
            title: "Shell",
            status: "completed",
            raw_input: { cmd: "ls" },
          },
          {
            kind: "tool_call",
            id: "call-2",
            title: "Read",
            status: "in_progress",
            content: "line1\\nline2",
          },
        ],
      },
    ]);

    expect(html).toContain("Tool Calls (2)");
    expect(html).toContain("acp-tool-group-fold");
    expect(html).toContain("acp-tool-group-list");
    expect(html).toContain("#1 Shell");
    expect(html).toContain("#2 Read");
    expect(html).toContain("tool-call-enter");
    expect(html).toContain("1 running");
    expect(html).toContain("acp-tool-group-status tone-running");
  });

  it("renders completed grouped tool status with success tone", () => {
    const html = renderConversation([
      {
        kind: "tool_call_group",
        event_id: 30,
        calls: [
          {
            kind: "tool_call",
            id: "call-a",
            title: "Search",
            status: "completed",
          },
          {
            kind: "tool_call",
            id: "call-b",
            title: "Read",
            status: "completed",
          },
        ],
      },
    ]);

    expect(html).toContain("2 completed");
    expect(html).toContain("acp-tool-group-status tone-completed");
  });

  it("renders explore groups as a single fold with thinking and nested tool calls", () => {
    const html = renderConversation([
      {
        kind: "explore_group",
        event_id: 20,
        items: [
          {
            kind: "agent_thinking",
            text: "explore **repo** layout",
            event_id: 18,
          },
          {
            kind: "tool_call_group",
            event_id: 19,
            calls: [
              {
                kind: "tool_call",
                id: "call-1",
                title: "Search",
                status: "completed",
              },
              {
                kind: "tool_call",
                id: "call-2",
                title: "Read",
                status: "in_progress",
                raw_input: { path: "README.md" },
              },
            ],
          },
        ],
      },
    ]);

    expect(html).toContain("acp-explore-group-fold");
    expect(html).toContain("Explore (2 tools)");
    expect(html).toContain("Explore #1");
    expect(html).toContain("<strong>repo</strong>");
    expect(html).toContain("#1 Search");
    expect(html).toContain("#2 Read");
    expect(html).toContain("README.md");
    expect(html).toContain("1 running");
  });

  it("renders JSON-like payload strings as structured sections", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-json-string",
        title: "Search",
        status: "completed",
        raw_input: "{\"query\":\"agenthub\",\"limit\":2}",
        raw_output: "[{\"path\":\"src/main.rs\"}]",
      },
    ]);

    expect(html).toContain("Tool Call: Search");
    expect(html).toContain("Completed");
    expect(html).toContain("agenthub");
    expect(html).toContain("path");
    expect(html).toContain("src/main.rs");
  });

  it("renders markdown code fences in tool text sections", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-markdown-code",
        title: "Write",
        status: "completed",
        raw_output: "```bash\necho hello\n```",
      },
    ]);

    expect(html).toContain("Tool Call: Write");
    expect(html).toContain("hljs-built_in\">echo</span>");
  });

  it("renders unified diff payloads with visual diff classes", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-diff-view",
        title: "Apply Patch",
        status: "completed",
        raw_output: [
          "diff --git a/src/a.ts b/src/a.ts",
          "index 111..222 100644",
          "--- a/src/a.ts",
          "+++ b/src/a.ts",
          "@@ -1,2 +1,2 @@",
          "-const oldValue = 1;",
          "+const newValue = 2;",
          " console.log(newValue);",
        ].join("\n"),
      },
    ]);

    expect(html).toContain("acp-diff-view");
    expect(html).toContain("acp-diff-line add");
    expect(html).toContain("acp-diff-line remove");
    expect(html).toContain("const oldValue = 1;");
    expect(html).toContain("const newValue = 2;");
  });

  it("preserves ascii-like text blocks without wrapping classes", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-ascii",
        title: "Render",
        status: "in_progress",
        content: [
          "   /\\_/\\",
          "  ( o.o )",
          "   > ^ <",
        ].join("\n"),
      },
    ]);

    expect(html).toContain("acp-payload-ascii");
    expect(html).toContain("( o.o )");
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
          status: "in_progress",
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
          status: "in_progress",
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

  it("shows success status dot for completed tool calls", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-success-mark",
        title: "Search",
        status: "completed",
      },
    ]);

    expect(html).toContain("acp-tool-status-mark tone-success");
    expect(html).toContain("acp-tool-status-dot");
    expect(html).toContain("aria-label=\"Completed\"");
  });

  it("shows error status dot for failed tool calls", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-error-mark",
        title: "Read",
        status: "failed",
      },
    ]);

    expect(html).toContain("acp-tool-status-mark tone-failure");
    expect(html).toContain("acp-tool-status-dot");
    expect(html).toContain("aria-label=\"Failed\"");
  });

  it("shows segmented footer for long tool text payloads", () => {
    const lines = Array.from({ length: 400 }, (_, idx) => `line-${idx}`).join("\n");
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-long-text",
        title: "Shell",
        status: "in_progress",
        content: lines,
      },
    ]);

    expect(html).toContain("Show more");
    expect(html).toContain("more lines");
    expect(html).toContain("line-0");
  });

  it("shows segmented footer for large structured payloads", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-large-payload",
        title: "Search",
        status: "completed",
        raw_output: {
          results: Array.from({ length: 120 }, (_, idx) => ({
            id: idx,
            path: `src/file-${idx}.ts`,
          })),
        },
      },
    ]);

    expect(html).toContain("Show more");
    expect(html).toContain("more items");
    expect(html).toContain("results");
  });

  it("hides debug-only payload fields such as turn_id/process_id/source", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-hidden-debug-fields",
        title: "Search",
        status: "completed",
        raw_output: {
          turn_id: "t-1",
          turnId: "t-2",
          process_id: "p-1",
          source: "agenthub",
          query: "agenthub",
          path: "src/main.rs",
        },
      },
    ]);

    expect(html).toContain("query");
    expect(html).toContain("agenthub");
    expect(html).toContain("path");
    expect(html).toContain("src/main.rs");
    expect(html).not.toContain("turn_id");
    expect(html).not.toContain("turnId");
    expect(html).not.toContain("process_id");
    expect(html).not.toContain("source");
    expect(html).not.toContain("t-1");
    expect(html).not.toContain("t-2");
    expect(html).not.toContain("p-1");
  });

  it("hides empty stderr/stdout fields while keeping non-empty stream output", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-hide-empty-stream-fields",
        title: "Shell",
        status: "completed",
        raw_output: {
          stdout: "line-1\\nline-2",
          stderr: "   \\n\\t",
          summary: "done",
        },
      },
    ]);

    expect(html).toContain("<dt>stdout</dt>");
    expect(html).toContain("line-1");
    expect(html).toContain("<dt>summary</dt>");
    expect(html).toContain("done");
    expect(html).not.toContain("<dt>stderr</dt>");
  });

  it("hides empty stderr/stdout fields for JSON-like string payloads", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-hide-empty-stream-fields-json-text",
        title: "Shell",
        status: "completed",
        raw_output:
          "{\"stdout\":\"\",\"stderr\":\"   \",\"summary\":\"done\"}",
      },
    ]);

    expect(html).not.toContain("<dt>stdout</dt>");
    expect(html).not.toContain("<dt>stderr</dt>");
    expect(html).toContain("<dt>summary</dt>");
    expect(html).toContain("done");
  });

  it("shows only highest-priority output field among aggregated/formatted/stdout", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-output-priority-object",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: "aggregated-value",
          formatted_output: "formatted-value",
          stdout: "stdout-value",
        },
      },
    ]);

    expect(html).toContain("<dt>aggregated_output</dt>");
    expect(html).toContain("aggregated-value");
    expect(html).not.toContain("<dt>formatted_output</dt>");
    expect(html).not.toContain("formatted-value");
    expect(html).not.toContain("<dt>stdout</dt>");
    expect(html).not.toContain("stdout-value");
  });

  it("falls back from aggregated_output to formatted_output when aggregated_output is empty", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-output-priority-json",
        title: "Shell",
        status: "completed",
        raw_output:
          "{\"aggregated_output\":\"   \",\"formatted_output\":\"formatted-value\",\"stdout\":\"stdout-value\"}",
      },
    ]);

    expect(html).not.toContain("<dt>aggregated_output</dt>");
    expect(html).toContain("<dt>formatted_output</dt>");
    expect(html).toContain("formatted-value");
    expect(html).not.toContain("<dt>stdout</dt>");
    expect(html).not.toContain("stdout-value");
  });

  it("falls back to stdout when aggregated_output and formatted_output are empty", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-output-priority-stdout-fallback",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: "",
          formatted_output: "   ",
          stdout: "stdout-value",
        },
      },
    ]);

    expect(html).not.toContain("<dt>aggregated_output</dt>");
    expect(html).not.toContain("<dt>formatted_output</dt>");
    expect(html).toContain("<dt>stdout</dt>");
    expect(html).toContain("stdout-value");
  });

  it("hides debug-only fields for JSON-like string payloads as well", () => {
    const html = renderConversation([
      {
        kind: "tool_call",
        id: "call-hidden-debug-fields-json-text",
        title: "Search",
        status: "completed",
        raw_output:
          "{\"turn_id\":\"t-3\",\"process_id\":\"p-2\",\"source\":\"agenthub\",\"query\":\"agenthub\",\"path\":\"src/app.tsx\"}",
      },
    ]);

    expect(html).toContain("query");
    expect(html).toContain("agenthub");
    expect(html).toContain("path");
    expect(html).toContain("src/app.tsx");
    expect(html).not.toContain("turn_id");
    expect(html).not.toContain("process_id");
    expect(html).not.toContain("source");
    expect(html).not.toContain("t-3");
    expect(html).not.toContain("p-2");
  });

  it("keeps thinking expanded while still rendering plan summary folds", () => {
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

    expect(html).toContain("acp-thinking-title");
    expect(html).toContain("Thinking");
    expect(html).not.toContain("agent_thinking\"><details");
    expect(html).toContain("Plan:");
  });

  it("renders thinking bubble content with markdown formatting", () => {
    const html = renderConversation([
      {
        kind: "agent_thinking",
        text: "**inspect** `query`",
        live: true,
        event_id: 20,
      },
    ]);

    expect(html).toContain("<strong>inspect</strong>");
    expect(html).toContain("<code>query</code>");
  });

  it("renders skill xml fragments as structured markdown fields", () => {
    const html = renderConversation([
      {
        kind: "agent_thinking",
        text: [
          "<skill>",
          "<name>team-leader-orchestrator</name>",
          "<path>/tmp/team-leader-orchestrator.SKILL.md</path>",
          "</skill>",
        ].join("\n"),
        live: false,
        event_id: 21,
      },
    ]);

    expect(html).toContain("<strong>Skill</strong>");
    expect(html).toContain("Name:");
    expect(html).toContain("<code>team-leader-orchestrator</code>");
    expect(html).toContain("Path:");
    expect(html).toContain("<code>/tmp/team-leader-orchestrator.SKILL.md</code>");
  });

  it("keeps skill xml inline-code rendering safe when fields contain backticks or html", () => {
    const html = renderConversation([
      {
        kind: "agent_thinking",
        text: [
          "<skill>",
          "<name>owner`name</name>",
          "<path>/tmp/``skill``-path/<img src=x onerror=alert(1)></path>",
          "</skill>",
        ].join("\n"),
        live: false,
        event_id: 22,
      },
    ]);

    expect(html).toContain("<strong>Skill</strong>");
    expect((html.match(/<code>/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
    expect(html).toContain("owner`name");
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
    expect(html).toContain("1/2 completed");
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

  it("renders markdown list, table, and code blocks in conversation bubbles", () => {
    const html = renderConversation([
      {
        kind: "agent_message",
        text: [
          "- item a",
          "- item b",
          "",
          "| col | value |",
          "| --- | --- |",
          "| k1 | v1 |",
          "",
          "Inline `code` sample.",
          "",
          "```ts",
          "const n = 1;",
          "```",
        ].join("\n"),
        event_id: 10,
      },
    ]);

    expect(html).toContain("<ul>");
    expect(html).toContain("<li>item a</li>");
    expect(html).toContain("<table>");
    expect(html).toContain("<th>col</th>");
    expect(html).toContain("<td>v1</td>");
    expect(html).toContain("<code>code</code>");
    expect(html).toContain("<pre class=\"hljs\"><code>");
    expect(html).toContain("hljs-keyword\">const</span>");
    expect(html).toContain("hljs-number\">1</span>");
  });
});
