import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamThreadPane } from "./team_thread_pane";

describe("TeamThreadPane", () => {
  it("renders the thread shell around the selected root message", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# all"
          rootMessageId={42}
          rootAuthorLabel="leader"
          rootCreatedAt={1713480000000}
          rootText="Investigate the regression in a focused thread."
          replies={[
            {
              messageId: 43,
              authorLabel: "worker-agent",
              createdAt: 1713480060000,
              text: "I can take the follow-up from here.",
            },
          ]}
          replyDraft="Draft follow-up"
          onReplyDraftChange={vi.fn()}
          onSendReply={vi.fn()}
          replyBusy={false}
          formatTs={() => "2026/4/19 00:00:00"}
          onViewInChannel={vi.fn()}
          onClose={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Thread");
    expect(html).toContain("# all");
    expect(html).toContain("View in channel");
    expect(html).toContain("Close thread");
    expect(html).toContain("Original message");
    expect(html).toContain("leader");
    expect(html).toContain("#42");
    expect(html).toContain("Investigate the regression in a focused thread.");
    expect(html).toContain("Replies stay scoped to this thread.");
    expect(html).toContain("worker-agent");
    expect(html).toContain("#43");
    expect(html).toContain("I can take the follow-up from here.");
    expect(html).toContain("Draft follow-up");
    expect(html).toContain("Reply");
    expect(html).toContain("max-w-[340px]");
    expect(html).toContain("rounded-[12px]");
    expect(html).toContain("hover:border-black");
  });

  it("keeps the reply composer available when the root has no chat text body", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# review"
          rootMessageId={51}
          rootAuthorLabel="planner"
          rootCreatedAt={1713480000000}
          rootText={null}
          replies={[]}
          replyDraft=""
          onReplyDraftChange={vi.fn()}
          onSendReply={vi.fn()}
          replyBusy={false}
          formatTs={() => "2026/4/19 00:00:00"}
          onViewInChannel={vi.fn()}
          onClose={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain("Original content is not available in chat text form.");
    expect(html).toContain("Reply in # review");
    expect(html).toContain("Reply");
  });
});
