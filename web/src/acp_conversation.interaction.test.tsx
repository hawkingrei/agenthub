// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AcpConversation } from "./components/acp_conversation";
import { ConversationItem } from "./conversation";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function setDetailsOpen(details: HTMLDetailsElement, open: boolean) {
  act(() => {
    details.open = open;
    details.dispatchEvent(new Event("toggle", { bubbles: true }));
  });
}

describe("AcpConversation fold interactions", () => {
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

  it("collapses input/output subfolds when parent tool call fold is collapsed", () => {
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-1",
        title: "Shell",
        status: "in_progress",
        raw_input: { cmd: "ls" },
        raw_output: { stdout: "line-1" },
      },
    ];

    act(() => {
      root.render(
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
          ansi={(input) => input}
        />
      );
    });

    const toolFold = container.querySelector(".acp-tool-fold") as HTMLDetailsElement | null;
    expect(toolFold).not.toBeNull();
    if (!toolFold) return;

    const findSubfold = (label: string): HTMLDetailsElement => {
      const fold = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
        const firstSpan = node.querySelector("summary span");
        return firstSpan?.textContent?.trim() === label;
      }) as HTMLDetailsElement | undefined;
      if (!fold) throw new Error(`subfold not found: ${label}`);
      return fold;
    };

    const inputFold = findSubfold("Input");
    const outputFold = findSubfold("Output");
    setDetailsOpen(inputFold, true);
    setDetailsOpen(outputFold, true);
    expect(inputFold.open).toBe(true);
    expect(outputFold.open).toBe(true);

    setDetailsOpen(toolFold, false);
    expect(findSubfold("Input").open).toBe(false);
    expect(findSubfold("Output").open).toBe(false);

    setDetailsOpen(toolFold, true);
    expect(findSubfold("Input").open).toBe(false);
    expect(findSubfold("Output").open).toBe(false);
  });

  it("shows newest lines first for long payloads and reveals older lines after Show more", () => {
    const lines = Array.from({ length: 260 }, (_, idx) => `line-${idx}`).join("\n");
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-tail-window",
        title: "Shell",
        status: "in_progress",
        content: lines,
      },
    ];

    act(() => {
      root.render(
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
          ansi={(input) => input}
        />
      );
    });

    const contentFold = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
      const firstSpan = node.querySelector("summary span");
      return firstSpan?.textContent?.trim() === "Content";
    }) as HTMLDetailsElement | undefined;
    expect(contentFold).not.toBeUndefined();
    if (!contentFold) return;
    setDetailsOpen(contentFold, true);

    const beforePre = container.querySelector("pre.acp-content.acp-payload-text");
    expect(beforePre).not.toBeNull();
    const preTextBeforeExpand = beforePre?.textContent ?? "";
    expect(preTextBeforeExpand).toContain("line-259");
    expect(preTextBeforeExpand).not.toContain("line-0");

    const showMoreButton = Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.includes("Show more")
    );
    expect(showMoreButton).not.toBeUndefined();
    if (!showMoreButton) return;

    act(() => {
      showMoreButton.click();
    });

    const afterPre = container.querySelector("pre.acp-content.acp-payload-text");
    expect(afterPre).not.toBeNull();
    const preTextAfterExpand = afterPre?.textContent ?? "";
    expect(preTextAfterExpand).toContain("line-104");
    expect(preTextAfterExpand).toContain("line-259");
    expect(preTextAfterExpand).not.toContain("line-0");
  });

  it("keeps Detailed collapsed by default after output fold disappears on rerender", () => {
    const baseProps = {
      windowOffset: 0,
      isFrozenView: false,
      shouldAutoCollapse: false,
      collapseCutoff: 0,
      runStatus: null,
      virtualTopSpacer: 0,
      virtualBottomSpacer: 0,
      stickToBottom: true,
      pendingCount: 0,
      avgHeight: 40,
      onScroll: () => {},
      containerRef: React.createRef<HTMLDivElement>(),
      ansi: (input: string) => input,
    };
    const firstItems: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-detailed-default-collapsed",
        title: "Shell",
        status: "completed",
        raw_output: { stdout: "line-1" },
      },
    ];

    act(() => {
      root.render(<AcpConversation items={firstItems} {...baseProps} />);
    });

    const secondItems: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-detailed-default-collapsed",
        title: "Shell",
        status: "completed",
        raw_output: {
          call_id: "call-detailed-default-collapsed",
          cwd: "/tmp/work",
          success: true,
        },
      },
    ];

    act(() => {
      root.render(<AcpConversation items={secondItems} {...baseProps} />);
    });

    const detailedFold = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
      const firstSpan = node.querySelector("summary span");
      return firstSpan?.textContent?.trim() === "Detailed";
    }) as HTMLDetailsElement | undefined;

    expect(detailedFold).not.toBeUndefined();
    expect(detailedFold?.open).toBe(false);
  });
});
