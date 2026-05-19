import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToReadableStream } from "react-dom/server";
import { beforeAll, describe, expect, it } from "vitest";
import { AcpConversation } from "./components/acp_conversation";
import { preloadThreadMarkdownAssets } from "./components/thread_rich_text";
import { ConversationItem } from "./conversation";

async function renderConversation(
  items: ConversationItem[],
  override?: Partial<React.ComponentProps<typeof AcpConversation>>
): Promise<string> {
  const stream = await renderToReadableStream(
    <MantineProvider>
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
        bottomAlignLatest={false}
        pendingCount={0}
        avgHeight={40}
        onScroll={() => {}}
        containerRef={React.createRef<HTMLDivElement>()}
        ansi={(input) => `<span class="ansi-out">${input}</span>`}
        {...override}
      />
    </MantineProvider>
  );
  await stream.allReady;
  return (await new Response(stream).text()).split("<!-- -->").join("");
}

describe("AcpConversation rendering", () => {
  beforeAll(async () => {
    await preloadThreadMarkdownAssets();
  });

  it("renders live tool calls expanded and terminal output through ansi renderer", async () => {
    const html = await renderConversation([
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
    expect(html).not.toContain("&lt;span class=&quot;ansi-out&quot;");
    expect(html).toContain("stdout");
  });

  it("uses terminal run status for stale live tool calls", async () => {
    const html = await renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-stale",
          title: "Shell",
          status: "in_progress",
          raw_input: { cmd: "cargo test" },
        },
      ],
      { runStatus: "stopped" }
    );

    expect(html).toContain("Tool Call: Shell");
    expect(html).toContain("Stopped");
    expect(html).not.toContain("In Progress");
  });

  it("renders terminal background activity as a dedicated ACP section", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-terminal-activity",
        title: "Run cargo test -p codex-core",
        status: "in_progress",
        terminal_activities: [
          {
            kind: "waited",
            command: "Run cargo test -p codex-core",
          },
          {
            kind: "interacted",
            command: "Run cargo test -p codex-core",
          },
        ],
      },
    ]);

    expect(html).toContain("Activity");
    expect(html).toContain("Terminal closed · Run cargo test -p codex-core");
    expect(html).toContain("Interacted with terminal · Run cargo test -p codex-core");
  });

  it("bottom-aligns the latest content when requested", async () => {
    const html = await renderConversation(
      [{ kind: "agent_message", text: "latest", event_id: 1 }],
      { bottomAlignLatest: true }
    );

    expect(html).toContain("acp-conversation-inner flex w-full flex-col gap-1 min-h-full justify-end");
  });

  it("keeps live tool calls collapsed when requested by the surface", async () => {
    const html = await renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-collapsed",
          title: "Shell",
          status: "in_progress",
          raw_input: { cmd: "cargo test" },
        },
      ],
      { toolCallsDefaultCollapsed: true }
    );

    expect(html).toContain("Tool Call: Shell");
    expect(html).toContain("In Progress");
    expect(html).toContain("acp-tool-fold");
    expect(html).not.toContain("open");
  });

  it("keeps grouped live tool calls collapsed when requested by the surface", async () => {
    const html = await renderConversation(
      [
        {
          kind: "tool_call_group",
          event_id: 12,
          calls: [
            {
              kind: "tool_call",
              id: "call-1",
              title: "Shell",
              status: "in_progress",
              raw_input: { cmd: "cargo test" },
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
        {
          kind: "explore_group",
          event_id: 13,
          items: [
            {
              kind: "agent_thinking",
              text: "inspect code",
              event_id: 14,
            },
            {
              kind: "tool_call",
              id: "call-3",
              title: "Search",
              status: "in_progress",
            },
          ],
        },
      ],
      { toolCallsDefaultCollapsed: true }
    );

    expect(html).toContain("Tool Calls (2)");
    expect(html).toContain("Explore (1 tool)");
    expect(html).toContain("acp-tool-group-fold");
    expect(html).toContain("acp-explore-group-fold");
    expect(html).not.toContain("open");
  });

  it("renders grouped tool calls with a shared fold and nested tool entries", async () => {
    const html = await renderConversation([
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
  });

  it("renders explore groups as a single fold with thinking and nested tool calls", async () => {
    const html = await renderConversation([
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
    expect(html).toContain("Explore #1 · explore repo layout");
    expect(html).toContain("acp-thinking-fold");
    expect(html).toContain("<strong>repo</strong>");
    expect(html).toContain("#1 Search");
    expect(html).toContain("#2 Read");
    expect(html).toContain("README.md");
    expect(html).toContain("1 running");
  });

  it("renders JSON-like payload strings as structured sections", async () => {
    const html = await renderConversation([
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

  it("renders agent and user messages with explicit bubble markers", async () => {
    const html = await renderConversation([
      {
        kind: "agent_message",
        text: "Agent reply",
        event_id: 1,
      },
      {
        kind: "user_message",
        text: "User prompt",
        event_id: 2,
      },
    ]);

    expect(html).toContain('data-acp-message-bubble="agent"');
    expect(html).toContain('data-acp-message-bubble="user"');
  });

  it("renders a native request_user_input card for pending questions", async () => {
    const html = await renderConversation(
      [
        {
          kind: "tool_call",
          id: "request-user-input:call-1",
          title: "Reasoning scope",
          status: "pending",
          raw_input: [
            {
              id: "scope",
              header: "Reasoning scope",
              question: "Which reasoning scope should I use?",
              isOther: false,
              isSecret: false,
              options: [
                {
                  label: "Plan only",
                  description: "Update only Plan mode.",
                },
              ],
            },
          ],
          content: "Codex needs input before continuing.",
        },
      ],
      {
        onSubmitRequestUserInput: () => {},
      }
    );

    expect(html).toContain("Input Required");
    expect(html).toContain("Submit your answer to continue execution.");
    expect(html).toContain("Submit Answer");
    expect(html).toContain("Plan only");
    expect(html).toContain('aria-labelledby="request-user-input:call-1:scope:header request-user-input:call-1:scope:prompt"');
    expect(html).toContain('id="request-user-input:call-1:scope:note"');
    expect(html).not.toContain(">Input<");
  });

  it("renders a native request_user_input result card for completed answers", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "request-user-input:call-2",
        title: "Question",
        status: "completed",
        raw_input: [
          {
            id: "scope",
            header: "Reasoning scope",
            question: "Which reasoning scope should I use?",
            isOther: false,
            isSecret: false,
            options: [
              {
                label: "Plan only",
                description: "Update only Plan mode.",
              },
            ],
          },
          {
            id: "notes",
            header: "Notes",
            question: "Add extra context.",
            isOther: false,
            isSecret: false,
          },
        ],
        raw_output: {
          answers: {
            scope: {
              answers: ["Plan only"],
            },
            notes: {
              answers: ["Need a narrower reasoning budget."],
            },
          },
        },
      },
    ]);

    expect(html).toContain("Questions answered");
    expect(html).toContain("2/2 answers recorded");
    expect(html).toContain("Need a narrower reasoning budget.");
    expect(html).not.toContain(">Output<");
    expect(html).not.toContain(">Input<");
  });

  it("renders secret request_user_input results as private placeholders", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "request-user-input:call-3",
        title: "Question",
        status: "completed",
        raw_input: [
          {
            id: "secret_scope",
            header: "Private scope",
            question: "Share the private scope.",
            isOther: false,
            isSecret: true,
          },
        ],
      },
    ]);

    expect(html).toContain("Question answered");
    expect(html).toContain("Answer submitted privately.");
    expect(html).toContain("suppressed the structured answer payload");
  });

  it("renders markdown code fences in tool text sections", async () => {
    const html = await renderConversation([
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

  it("renders markdown lists in tool text sections without falling back to plain pre blocks", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-markdown-list",
        title: "Summarize",
        status: "completed",
        raw_output: "- first item\n- second item\n- third item",
      },
    ]);

    expect(html).toContain("Tool Call: Summarize");
    expect(html).toContain('<ul class="md-list md-list-unordered">');
    expect(html).toContain('<li class="md-list-item">first item</li>');
    expect(html).not.toContain("acp-payload-text");
  });

  it("renders markdown in tool content sections", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-markdown-content",
        title: "Analyze",
        status: "completed",
        content: "## Findings\n- one\n- two",
      },
    ]);

    expect(html).toContain("Tool Call: Analyze");
    expect(html).toContain('<h2 class="md-heading md-h2">Findings</h2>');
    expect(html).toContain('<li class="md-list-item">one</li>');
    expect(html).not.toContain("## Findings");
  });

  it("renders unified diff payloads with visual diff classes", async () => {
    const html = await renderConversation([
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

  it("preserves ascii-like text blocks without wrapping classes", async () => {
    const html = await renderConversation([
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

  it("sanitizes terminal html while keeping allowed ansi span tags", async () => {
    const html = await renderConversation(
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

  it("keeps nested ansi style spans and combines inherited styles", async () => {
    const html = await renderConversation(
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

  it("drops unsupported ansi styles and strips non-style span attributes while keeping text", async () => {
    const html = await renderConversation(
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
    expect(html).toContain("literal");
    expect(html).not.toContain("&lt;span class=&quot;ansi-out&quot;&gt;");
  });

  it("renders finished tool calls collapsed by default", async () => {
    const html = await renderConversation([
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

  it("collapses older live tool calls once the conversation passes the cutoff window", async () => {
    const html = await renderConversation(
      [
        {
          kind: "tool_call",
          id: "call-live-cutoff",
          title: "Shell",
          status: "in_progress",
          raw_input: { cmd: "cargo test" },
        },
      ],
      {
        shouldAutoCollapse: true,
        collapseCutoff: 10,
      }
    );

    expect(html).toContain("Tool Call: Shell");
    expect(html).not.toMatch(/acp-tool-fold" open/);
  });

  it("collapses older live grouped cards once the conversation passes the cutoff window", async () => {
    const html = await renderConversation(
      [
        {
          kind: "explore_group",
          event_id: 20,
          items: [
            {
              kind: "agent_thinking",
              text: "explore repo layout",
              event_id: 18,
            },
            {
              kind: "tool_call",
              id: "call-live-group-cutoff",
              title: "Search",
              status: "in_progress",
            },
          ],
        },
      ],
      {
        shouldAutoCollapse: true,
        collapseCutoff: 10,
      }
    );

    expect(html).toContain("Explore (1 tool)");
    expect(html).not.toMatch(/acp-explore-group-fold" open/);
  });

  it("shows success status dot for completed tool calls", async () => {
    const html = await renderConversation([
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

  it("shows error status dot for failed tool calls", async () => {
    const html = await renderConversation([
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

  it("shows duration next to tool status when raw_output provides duration", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-duration-status",
        title: "Shell",
        status: "completed",
        raw_output: {
          duration: {
            secs: 2,
            nanos: 250_000_000,
          },
        },
      },
    ]);

    expect(html).toContain("Completed");
    expect(html).toContain("2.25s");
    expect(html).not.toContain("<dt>duration</dt>");
  });

  it("shows segmented footer for long tool text payloads", async () => {
    const lines = Array.from({ length: 400 }, (_, idx) => `line-${idx}`).join("\n");
    const html = await renderConversation([
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
    expect(html).toContain("acp-terminal-pre");
    expect(html).toContain("acp-content acp-payload-text");
    expect(html).toContain(">line-364");
    expect(html).toContain("line-399");
    expect(html).not.toContain(">line-363");
  });

  it("renders aggregated_output in tail-first mode and keeps older lines behind Show more", async () => {
    const lines = Array.from({ length: 400 }, (_, idx) => `agg-${idx}`).join("\n");
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-aggregated-tail",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: lines,
        },
      },
    ]);

    expect(html).toContain("<dt>aggregated_output</dt>");
    expect(html).toContain("Show more");
    expect(html).toContain('<pre class="acp-content acp-payload-text');
    expect(html).toContain(">agg-364");
    expect(html).toContain("agg-399");
    expect(html).not.toContain(">agg-363");
  });

  it("renders aggregated_output as plain terminal block even when text looks like markdown", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-aggregated-markdown-like",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: "```bash\necho hello\n```",
        },
      },
    ]);

    expect(html).toContain("<dt>aggregated_output</dt>");
    expect(html).toContain('<pre class="acp-content acp-payload-text');
    expect(html).toContain("```bash");
    expect(html).toContain("echo hello");
    expect(html).not.toContain("acp-payload-markdown");
  });

  it("truncates overlong html class attributes inside plain text payloads", async () => {
    const longClassValue = [
      "inline-flex",
      "items-center",
      "justify-center",
      "rounded-lg",
      "font-semibold",
      "transition",
      "active:translate-y-px",
      "disabled:cursor-not-allowed",
      "disabled:opacity-50",
      "shadow-sm",
      "hover:bg-notion-hover",
      "hover:text-notion-text",
      "backdrop-blur-[2px]",
      "hover:border-black/10",
      "hover:bg-white",
    ].join(" ");
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-long-class-attribute",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: `<div class="${longClassValue}">content</div>`,
        },
      },
    ]);

    expect(html).toContain('class=&quot;inline-flex items-center justify-center rounded-lg font-semibold transi…&quot;');
    expect(html).toContain(`title="&lt;div class=&quot;${longClassValue}&quot;&gt;content&lt;/div&gt;"`);
    expect(html).toContain("&lt;div");
    expect(html).toContain("content");
  });

  it("does not truncate plain text that only mentions class attributes outside html tags", async () => {
    const longClassValue = [
      "inline-flex",
      "items-center",
      "justify-center",
      "rounded-lg",
      "font-semibold",
      "transition",
      "active:translate-y-px",
      "disabled:cursor-not-allowed",
      "disabled:opacity-50",
      "shadow-sm",
    ].join(" ");
    const plainText = `Debug note: use class="${longClassValue}" on the wrapper`;
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-class-mention",
        title: "Shell",
        status: "completed",
        raw_output: {
          aggregated_output: plainText,
        },
      },
    ]);

    expect(html).toContain(`Debug note: use class=&quot;${longClassValue}&quot; on the wrapper`);
    expect(html).not.toContain("transi…");
  });

  it("shows segmented footer for large structured payloads", async () => {
    const html = await renderConversation([
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

  it("hides debug-only payload fields such as turn_id/process_id/source", async () => {
    const html = await renderConversation([
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
    expect(html).not.toContain(">t-1<");
    expect(html).not.toContain(">t-2<");
    expect(html).not.toContain(">p-1<");
  });

  it("hides call_id/cwd/success from regular payload fields and shows them in Detailed section", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-hidden-in-payload",
        title: "Shell",
        status: "completed",
        raw_output: {
          call_id: "call-hidden-in-payload",
          cwd: "/tmp/work",
          success: true,
          duration_ms: 987,
          summary: "done",
        },
      },
    ]);

    expect(html).toContain("Detailed");
    expect(html).toContain("call-hidden-in-payload");
    expect(html).toContain("<dt>summary</dt>");
    const callIdRows = html.match(/<dt>call_id<\/dt>/g) ?? [];
    expect(callIdRows.length).toBe(1);
    const cwdRows = html.match(/<dt>cwd<\/dt>/g) ?? [];
    expect(cwdRows.length).toBe(1);
    const successRows = html.match(/<dt>success<\/dt>/g) ?? [];
    expect(successRows.length).toBe(1);
    expect(html).not.toContain("<dt>duration_ms</dt>");
  });

  it("shows only unified_diff in edit output and moves other fields to Detailed", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-edit-only-diff",
        title: "Edit",
        status: "completed",
        raw_output: {
          unified_diff: "--- a/file.ts\n+++ b/file.ts\n@@ -1 +1 @@\n-old\n+new",
          path: "src/file.ts",
          cwd: "/tmp/work",
          success: true,
          duration_ms: 1234,
          summary: "applied",
        },
      },
    ]);

    const diffRows = html.match(/<dt>unified_diff<\/dt>/g) ?? [];
    expect(diffRows.length).toBe(1);
    expect(html).toContain("+++ b/file.ts");

    const pathRows = html.match(/<dt>path<\/dt>/g) ?? [];
    expect(pathRows.length).toBe(1);
    const cwdRows = html.match(/<dt>cwd<\/dt>/g) ?? [];
    expect(cwdRows.length).toBe(1);
    const successRows = html.match(/<dt>success<\/dt>/g) ?? [];
    expect(successRows.length).toBe(1);
    const summaryRows = html.match(/<dt>summary<\/dt>/g) ?? [];
    expect(summaryRows.length).toBe(1);
    expect(html).not.toContain("<dt>duration_ms</dt>");
  });

  it("keeps edit old/new content in Detailed and hides edit output when unified_diff is missing", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-edit-old-new-detailed",
        title: "Edit",
        status: "completed",
        raw_output: {
          old_content: "line-old-1\\nline-old-2",
          new_content: "line-new-1\\nline-new-2",
          summary: "prepared",
        },
      },
    ]);

    expect(html).toContain("Detailed");
    expect(html).toContain("<dt>old_content</dt>");
    expect(html).toContain("<dt>new_content</dt>");
    expect(html).toContain("<dt>summary</dt>");
    expect(html).not.toContain("<dt>unified_diff</dt>");
    expect(html).not.toContain(">Output</span>");
  });

  it("hides empty stderr/stdout fields while keeping non-empty stream output", async () => {
    const html = await renderConversation([
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

  it("renders small nested arrays inline instead of collapsed Array(N) details", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-inline-small-array",
        title: "Search",
        status: "completed",
        raw_output: {
          results: [
            { path: "a.ts" },
            { path: "b.ts" },
            { path: "c.ts" },
          ],
        },
      },
    ]);

    expect(html).toContain("<dt>results</dt>");
    expect(html).toContain("a.ts");
    expect(html).toContain("b.ts");
    expect(html).toContain("c.ts");
    expect(html).not.toContain("Array(3)");
  });

  it("renders structured array payload without numeric list markers", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-structured-array-no-numbering",
        title: "Shell",
        status: "completed",
        raw_output: {
          parsed_cmd: Array.from({ length: 11 }, (_, idx) => ({
            arg: `arg-${idx}`,
          })),
        },
      },
    ]);

    expect(html).toContain("<dt>parsed_cmd</dt>");
    expect(html).toContain("arg-0");
    expect(html).toContain("<ul class=\"acp-payload-list");
    expect(html).not.toContain("<ol class=\"acp-payload-list");
    expect(html).not.toContain("list-decimal");
  });

  it("normalizes numeric-key objects into array-style payload rendering", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-numeric-key-object",
        title: "Shell",
        status: "completed",
        raw_output: {
          parsed_cmd: {
            1: { arg: "arg-1" },
            2: { arg: "arg-2" },
          },
        },
      },
    ]);

    expect(html).toContain("<dt>parsed_cmd</dt>");
    expect(html).toContain("arg-1");
    expect(html).toContain("arg-2");
    expect(html).not.toContain("<dt>1</dt>");
    expect(html).not.toContain("<dt>2</dt>");
  });

  it("renders payload context/content strings as plain text without markdown list numbering", async () => {
    const html = await renderConversation([
      {
        kind: "tool_call",
        id: "call-payload-context-plain-text",
        title: "Shell",
        status: "completed",
        raw_output: {
          context: "1. step one\\n2. step two",
          content: "1. item one\\n2. item two",
        },
      },
    ]);

    expect(html).toContain("<dt>context</dt>");
    expect(html).toContain("<dt>content</dt>");
    expect(html).toContain("1. step one");
    expect(html).toContain("2. item two");
    expect(html).not.toContain("<ol>");
  });

  it("hides empty stderr/stdout fields for JSON-like string payloads", async () => {
    const html = await renderConversation([
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

  it("shows only highest-priority output field among aggregated/formatted/stdout", async () => {
    const html = await renderConversation([
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

  it("falls back from aggregated_output to formatted_output when aggregated_output is empty", async () => {
    const html = await renderConversation([
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

  it("falls back to stdout when aggregated_output and formatted_output are empty", async () => {
    const html = await renderConversation([
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

  it("hides debug-only fields for JSON-like string payloads as well", async () => {
    const html = await renderConversation([
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
    expect(html).not.toMatch(/>p-2</);
  });

  it("renders thinking inside collapsed folds while preserving the first-line title", async () => {
    const html = await renderConversation(
      [
        {
          kind: "agent_thinking",
          text: "Plan next step\nthinking details for summary",
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

    expect(html).toContain("acp-thinking-fold");
    expect(html).toContain("THINKING · Plan next step");
    expect(html).not.toContain("acp-thinking-title");
    expect(html).toContain("Plan:");
  });

  it("renders thinking bubble content with markdown formatting", async () => {
    const html = await renderConversation([
      {
        kind: "agent_thinking",
        text: "**inspect** `query`",
        live: true,
        event_id: 20,
      },
    ]);

    expect(html).toContain("THINKING · inspect query (live)");
    expect(html).toContain("acp-thinking-fold");
    expect(html).toContain("<strong>inspect</strong>");
    expect(html).toContain('<code class="md-inline-code">query</code>');
  });

  it("renders skill xml fragments as structured markdown fields", async () => {
    const html = await renderConversation([
      {
        kind: "agent_thinking",
        text: [
          "<skill>",
          "<name>team-coordinator-orchestrator</name>",
          "<path>/tmp/team-coordinator-orchestrator.SKILL.md</path>",
          "</skill>",
        ].join("\n"),
        live: false,
        event_id: 21,
      },
    ]);

    expect(html).toContain("<strong>Skill</strong>");
    expect(html).toContain("Name:");
    expect(html).toContain(
      '<code class="md-inline-code">team-coordinator-orchestrator</code>'
    );
    expect(html).toContain("Path:");
    expect(html).toContain(
      '<code class="md-inline-code">/tmp/team-coordinator-orchestrator.SKILL.md</code>'
    );
  });

  it("keeps skill xml inline-code rendering safe when fields contain backticks or html", async () => {
    const html = await renderConversation([
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
    expect((html.match(/<code\b/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;img");
    expect(html).toContain("owner`name");
  });

  it("renders plan entries as a structured plan card", async () => {
    const html = await renderConversation([
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

  it("uses a non-empty collapsed plan summary when preview text is blank", async () => {
    const html = await renderConversation(
      [
        {
          kind: "agent_plan",
          text: "   ",
          event_id: 3,
        },
      ],
      {
        shouldAutoCollapse: true,
        collapseCutoff: 10,
      }
    );

    expect(html).toContain("Plan (collapsed)");
    expect(html).not.toContain("Plan: </summary>");
  });

  it("renders markdown bubbles and pending spacer", async () => {
    const html = await renderConversation(
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

  it("renders markdown list, table, and code blocks in conversation bubbles", async () => {
    const html = await renderConversation([
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

    expect(html).toContain('<ul class="md-list md-list-unordered">');
    expect(html).toContain('<li class="md-list-item">item a</li>');
    expect(html).toContain('<div class="md-table-wrap"><table class="md-table">');
    expect(html).toContain('<th class="md-table_th">col</th>');
    expect(html).toContain('<td class="md-table_td">v1</td>');
    expect(html).toContain('<code class="md-inline-code">code</code>');
    expect(html).toContain(
      '<pre class="md-code-block hljs" data-language="ts"><code class="md-code-block_code">'
    );
    expect(html).toContain("hljs-keyword\">const</span>");
    expect(html).toContain("hljs-number\">1</span>");
  });
});
