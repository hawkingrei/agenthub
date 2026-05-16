// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TeamMessageComposer } from "./team_message_composer";
import {
  TEAM_MESSAGE_COMPOSER_CONTEXT_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_ALIAS_CLASS,
  TEAM_MESSAGE_COMPOSER_MENTION_MENU_CLASS,
  TEAM_MESSAGE_COMPOSER_SHELL_CLASS,
} from "../../ui/tailwind_classes";

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

function expectStaticClassTokens(html: string, className: string): void {
  for (const token of className.split(/\s+/).filter(Boolean)) {
    expect(html).toContain(token.split("&").join("&amp;"));
  }
}

describe("TeamMessageComposer", () => {
  let container: HTMLDivElement;
  let root: Root;

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
  });

  it("renders a shared shell for channel and thread composers", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <TeamMessageComposer
          id="team-message"
          name="team_message"
          draft="hello"
          placeholder="Message #all"
          contextText="Full context in thread"
          helperText="@name to reply · Enter to send"
          sendLabel="Send"
          onDraftChange={vi.fn()}
          onSend={vi.fn()}
        />
      </MantineProvider>
    );

    expect(html).toContain('id="team-message"');
    expect(html).toContain('name="team_message"');
    expect(html).toContain("Message #all");
    expect(html).toContain("Full context in thread");
    expect(html).toContain("@name to reply · Enter to send");
    expect(html).toContain("Send");
    expectStaticClassTokens(html, TEAM_MESSAGE_COMPOSER_SHELL_CLASS);
    expectStaticClassTokens(html, TEAM_MESSAGE_COMPOSER_CONTEXT_CLASS);
  });

  it("renders mention options and delegates selection without blurring the textarea first", () => {
    const onSelectMention = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamMessageComposer
            draft="@wo"
            placeholder="Message #all"
            helperText="@name to reply · Enter to send"
            sendLabel="Send"
            mentionOptions={[
              {
                actorId: "worker-agent",
                label: "Worker Agent",
                aliases: ["worker-agent"],
              },
            ]}
            activeMentionIndex={0}
            onSelectMention={onSelectMention}
            onDraftChange={vi.fn()}
            onSend={vi.fn()}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Select teammate mention");
    expect(container.textContent).toContain("@Worker Agent");
    expect(container.innerHTML).toContain(TEAM_MESSAGE_COMPOSER_MENTION_MENU_CLASS);
    expect(container.innerHTML).toContain(TEAM_MESSAGE_COMPOSER_MENTION_ALIAS_CLASS);

    const option = container.querySelector('[data-team-mention-option="worker-agent"]');
    expect(option).not.toBeNull();
    act(() => {
      option?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    });

    expect(onSelectMention).toHaveBeenCalledWith({
      actorId: "worker-agent",
      label: "Worker Agent",
      aliases: ["worker-agent"],
    });
  });
});
