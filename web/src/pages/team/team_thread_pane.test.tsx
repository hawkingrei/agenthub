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
    expect(html).toContain("max-w-[340px]");
    expect(html).toContain("rounded-[12px]");
    expect(html).toContain("hover:border-black");
  });
});
