// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TeamThreadPane } from "./team_thread_pane";
import {
  TEAM_THREAD_CHANNEL_BADGE_CLASS,
  TEAM_THREAD_MESSAGE_AVATAR_CLASS,
  TEAM_THREAD_MESSAGE_BUBBLE_CLASS,
  TEAM_THREAD_PANE_CLASS,
  TEAM_THREAD_SOURCE_CARD_CLASS,
} from "../../ui/tailwind_classes";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function expectStaticClassTokens(html: string, className: string): void {
  for (const token of className.split(/\s+/).filter(Boolean)) {
    expect(html).toContain(token.split("&").join("&amp;"));
  }
}

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
          rootAuthorLabel="coordinator"
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

    expect(html).toContain("Reply in thread");
    expect(html).toContain("# all");
    expect(html).toContain("1 reply");
    expect(html).toContain(
      "Keep the root summary-first. Use the thread for detailed context, logs, and follow-up."
    );
    expect(html).toContain("Source");
    expect(html).toContain("From coordinator · #42");
    expect(html).toContain("Investigate the regression in a focused thread.");
    expect(html).toContain("View in channel");
    expect(html).toContain("Close thread");
    expect(html).toContain("Original");
    expect(html).toContain("Replies");
    expect(html).toContain("coordinator");
    expect(html).toContain('data-avatar-seed="coordinator::coordinator"');
    expect(html).toContain("#42");
    expect(html).toContain("Investigate the regression in a focused thread.");
    expect(html).toContain("Replies");
    expect(html).toContain("worker-agent");
    expect(html).toContain('data-avatar-seed="worker-agent::worker-agent"');
    expect(html).toContain("#43");
    expect(html).toContain("I can take the follow-up from here.");
    expect(html).toContain("Draft follow-up");
    expect(html).toContain("Reply in thread · # all");
    expect(html).toContain("@name to reply · Enter to reply");
    expect(html).toContain("Reply");
    expectStaticClassTokens(html, TEAM_THREAD_PANE_CLASS);
    expectStaticClassTokens(html, TEAM_THREAD_CHANNEL_BADGE_CLASS);
    expectStaticClassTokens(html, TEAM_THREAD_SOURCE_CARD_CLASS);
    expectStaticClassTokens(html, TEAM_THREAD_MESSAGE_AVATAR_CLASS);
    expectStaticClassTokens(html, TEAM_THREAD_MESSAGE_BUBBLE_CLASS);
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

  it("uses the root message id for unknown-author avatar seeds", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# review"
          rootMessageId={51}
          rootAuthorLabel={null}
          rootCreatedAt={1713480000000}
          rootText={null}
          replies={[
            {
              messageId: 52,
              authorLabel: null,
              createdAt: 1713480060000,
              text: "System follow-up",
            },
          ]}
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

    expect(html).toContain('data-avatar-seed="Unknown::51"');
    expect(html).toContain('data-avatar-seed="Unknown::52"');
  });

  it("truncates long source previews without changing the root thread content", () => {
    const longRootText =
      "This is a long source message that should stay fully visible in the original root bubble while the compact source strip only carries a short preview for context.";
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamThreadPane
          channelLabel="# review"
          rootMessageId={88}
          rootAuthorLabel="coordinator"
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

    expect(html).toContain("From coordinator · #88");
    expect(html).toContain("Replies");
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
            rootAuthorLabel="coordinator"
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
    expect(onSendReply).toHaveBeenCalledWith({
      text: "Ready to reply",
      mentionActorIds: [],
    });
    expect(container.textContent).toContain("@name to reply · Enter to reply");
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
            rootAuthorLabel="coordinator"
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
    expect(container.textContent).toContain("@name to reply · Enter adds a new line");
  });

  it("lets the thread composer select teammate mentions", () => {
    const onSendReply = vi.fn();
    const onReplyDraftChange = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamThreadPane
            channelLabel="# review"
            rootMessageId={88}
            rootAuthorLabel="coordinator"
            rootCreatedAt={1713480000000}
            rootText="Discuss here."
            replies={[]}
            replyDraft="@wo"
            onReplyDraftChange={onReplyDraftChange}
            onSendReply={onSendReply}
            replyBusy={false}
            mentionCandidates={[
              {
                actorId: "worker-agent",
                label: "Worker Agent",
                aliases: ["worker-agent"],
              },
            ]}
            formatTs={() => "2026/4/19 00:00:00"}
            onViewInChannel={vi.fn()}
            onClose={vi.fn()}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Select teammate mention");
    const option = container.querySelector('[data-team-mention-option="worker-agent"]');
    expect(option).not.toBeNull();
    act(() => {
      option?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    });
    expect(onReplyDraftChange).toHaveBeenCalledWith("@Worker Agent");
    expect(onSendReply).not.toHaveBeenCalled();
  });

  it("canonicalizes teammate mentions before sending a thread reply", () => {
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
            rootAuthorLabel="coordinator"
            rootCreatedAt={1713480000000}
            rootText="Discuss here."
            replies={[]}
            replyDraft="Ping @worker-agent now"
            onReplyDraftChange={vi.fn()}
            onSendReply={onSendReply}
            replyBusy={false}
            mentionCandidates={[
              {
                actorId: "worker-agent",
                label: "Worker Agent",
                aliases: ["worker-agent"],
              },
            ]}
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

    expect(onSendReply).toHaveBeenCalledWith({
      text: "Ping <at>worker-agent</at> now",
      mentionActorIds: ["worker-agent"],
    });
  });

  it("keeps keyboard mention selection when arrow navigation updates the active option", () => {
    const onReplyDraftChange = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamThreadPane
            channelLabel="# review"
            rootMessageId={88}
            rootAuthorLabel="coordinator"
            rootCreatedAt={1713480000000}
            rootText="Discuss here."
            replies={[]}
            replyDraft="@w"
            onReplyDraftChange={onReplyDraftChange}
            onSendReply={vi.fn()}
            replyBusy={false}
            mentionCandidates={[
              {
                actorId: "worker-agent",
                label: "Worker Agent",
                aliases: ["worker-agent"],
              },
              {
                actorId: "writer-agent",
                label: "Writer Agent",
                aliases: ["writer-agent"],
              },
            ]}
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
      textarea?.dispatchEvent(new FocusEvent("focus", { bubbles: true }));
      textarea?.setSelectionRange(2, 2);
      textarea?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      textarea?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true })
      );
      textarea?.dispatchEvent(
        new KeyboardEvent("keyup", { key: "ArrowDown", bubbles: true, cancelable: true })
      );
    });

    act(() => {
      textarea?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );
    });

    expect(onReplyDraftChange).toHaveBeenCalledWith("@Writer Agent");
  });
});
