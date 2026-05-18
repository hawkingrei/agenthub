// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TeamThreadRichText } from "./team_thread_rich_text";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "../../test_utils/react_test_helpers";

installReactDomTestGlobals();

describe("TeamThreadRichText", () => {
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

  it("renders sanitized team markdown html", () => {
    act(() => {
      renderWithMantine(
        root,
        <TeamThreadRichText text={`<script>alert(1)</script>\n<img src=x onerror=alert(1)>\n[unsafe](javascript:alert(1))`} />
      );
    });

    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector('a[href^="javascript:"]')).toBeNull();
    expect(container.innerHTML).toContain("&lt;script&gt;alert(1)&lt;/script&gt;");
    expect(container.innerHTML).toContain("&lt;img src=x onerror=alert(1)&gt;");
  });

  it("delegates mention chip clicks to the actor profile handler", () => {
    const onMentionClick = vi.fn();
    act(() => {
      renderWithMantine(
        root,
        <TeamThreadRichText
          text="@worker-1"
          renderSanitizedHtml={() =>
            '<button type="button" data-team-agent-mention-id="worker-1">@Worker</button>'
          }
          onMentionClick={onMentionClick}
        />
      );
    });

    const mention = container.querySelector(
      '[data-team-agent-mention-id="worker-1"]'
    ) as HTMLButtonElement | null;
    expect(mention).not.toBeNull();
    mention?.click();
    expect(onMentionClick).toHaveBeenCalledWith("worker-1");
  });

  it("ignores rich text clicks when they are not mention chips", () => {
    const onMentionClick = vi.fn();
    act(() => {
      renderWithMantine(
        root,
        <TeamThreadRichText
          text="plain"
          renderSanitizedHtml={() => "<p>plain</p>"}
          onMentionClick={onMentionClick}
        />
      );
    });

    container.querySelector("p")?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    expect(onMentionClick).not.toHaveBeenCalled();
  });
});
