// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TeamThreadPane } from "./team_thread_pane";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia;
}

describe("TeamThreadPane", () => {
  let container: HTMLDivElement;
  let root: Root;
  const originalWidth = window.innerWidth;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: originalWidth,
    });
  });

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
    expect(html).toContain("1 reply");
    expect(html).toContain("Focused replies stay anchored to the source message.");
    expect(html).toContain("Source");
    expect(html).toContain("From leader · #42");
    expect(html).toContain("Investigate the regression in a focused thread.");
    expect(html).toContain("View in channel");
    expect(html).toContain("Close thread");
    expect(html).toContain("Original");
    expect(html).toContain("Thread replies");
    expect(html).toContain("leader");
    expect(html).toContain(">L<");
    expect(html).toContain("#42");
    expect(html).toContain("Investigate the regression in a focused thread.");
    expect(html).toContain("Thread replies");
    expect(html).toContain("worker-agent");
    expect(html).toContain(">W<");
    expect(html).toContain("#43");
    expect(html).toContain("I can take the follow-up from here.");
    expect(html).toContain("Draft follow-up");
    expect(html).toContain("Reply in thread · # all");
    expect(html).toContain("Reply stays in this thread · Enter to reply");
    expect(html).toContain("Reply");
    expect(html).toContain("max-w-[360px]");
    expect(html).toContain("rounded-full");
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
    expect(html).toContain("0 replies");
    expect(html).toContain("Reply in thread · # review");
    expect(html).toContain("Reply");
  });

  it("truncates long source previews without changing the root thread content", () => {
    const longRootText =
      "This is a long source message that should stay fully visible in the original root bubble while the compact source strip only carries a short preview for context.";
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# review"
          rootMessageId={88}
          rootAuthorLabel="leader"
          rootCreatedAt={1713480000000}
          rootText={longRootText}
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

    expect(html).toContain("From leader · #88");
    expect(html).toContain("Thread replies");
    expect(html).toContain(
      "This is a long source message that should stay fully visible in the original root bubble while the compact source str..."
    );
    expect(html).toContain(longRootText);
  });

  it("shows the empty state when no thread root is selected", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# review"
          rootMessageId={null}
          rootAuthorLabel={null}
          rootCreatedAt={null}
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

    expect(html).toContain("Select a channel message");
    expect(html).toContain("Thread roots open from existing channel messages.");
    expect(html).not.toContain("Reply in thread · # review");
  });

  it("sends reply on Enter for desktop-sized viewports", () => {
    const onSendReply = vi.fn();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 1440,
    });

    act(() => {
      root.render(
        <MantineProvider>
          <TeamThreadPane
            channelLabel="# review"
            rootMessageId={88}
            rootAuthorLabel="leader"
            rootCreatedAt={1713480000000}
            rootText="Discuss here."
            replies={[]}
            replyDraft="Ready to reply"
            onReplyDraftChange={vi.fn()}
            onSendReply={onSendReply}
            replyBusy={false}
            formatTs={() => "2026/4/19 00:00:00"}
            onViewInChannel={vi.fn()}
            onClose={vi.fn()}
          />
        </MantineProvider>
      );
    });

    const textarea = container.querySelector("textarea");
    expect(textarea).not.toBeNull();
    act(() => {
      textarea?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );
    });

    expect(onSendReply).toHaveBeenCalledTimes(1);
    expect(container.textContent).toContain("Reply stays in this thread · Enter to reply");
  });

  it("keeps Enter as newline on mobile-sized viewports", () => {
    const onSendReply = vi.fn();
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      writable: true,
      value: 480,
    });

    act(() => {
      root.render(
        <MantineProvider>
          <TeamThreadPane
            channelLabel="# review"
            rootMessageId={88}
            rootAuthorLabel="leader"
            rootCreatedAt={1713480000000}
            rootText="Discuss here."
            replies={[]}
            replyDraft="Ready to reply"
            onReplyDraftChange={vi.fn()}
            onSendReply={onSendReply}
            replyBusy={false}
            formatTs={() => "2026/4/19 00:00:00"}
            onViewInChannel={vi.fn()}
            onClose={vi.fn()}
          />
        </MantineProvider>
      );
    });

    const textarea = container.querySelector("textarea");
    expect(textarea).not.toBeNull();
    act(() => {
      window.dispatchEvent(new Event("resize"));
    });
    act(() => {
      textarea?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );
    });

    expect(onSendReply).not.toHaveBeenCalled();
    expect(container.textContent).toContain("Reply stays in this thread · Enter adds a new line");
  });
});
