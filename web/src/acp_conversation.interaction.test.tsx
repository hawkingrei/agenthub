// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
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

function setNativeValue(
  element: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement,
  value: string
): void {
  const prototype = Object.getPrototypeOf(element) as {
    value?: unknown;
  };
  const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");
  if (descriptor?.set) {
    descriptor.set.call(element, value);
    return;
  }
  element.value = value;
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

  it("auto-collapses an older live tool call when it crosses the conversation cutoff", () => {
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-live-cutoff-transition",
        title: "Shell",
        status: "in_progress",
      },
    ];
    const baseProps = {
      windowOffset: 0,
      isFrozenView: false,
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

    act(() => {
      root.render(
        <AcpConversation
          items={items}
          shouldAutoCollapse={false}
          collapseCutoff={0}
          {...baseProps}
        />
      );
    });

    const toolFold = container.querySelector(".acp-tool-fold") as HTMLDetailsElement | null;
    expect(toolFold).not.toBeNull();
    expect(toolFold?.open).toBe(true);

    act(() => {
      root.render(
        <AcpConversation
          items={items}
          shouldAutoCollapse={true}
          collapseCutoff={10}
          {...baseProps}
        />
      );
    });

    const collapsedToolFold = container.querySelector(
      ".acp-tool-fold"
    ) as HTMLDetailsElement | null;
    expect(collapsedToolFold).not.toBeNull();
    expect(collapsedToolFold?.open).toBe(false);
  });

  it("submits multi-question request_user_input answers through the shared ACP input callback", async () => {
    const onSubmitRequestUserInput = vi.fn().mockResolvedValue(undefined);
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "request-user-input:call-1",
        title: "Question",
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
          {
            id: "notes",
            header: "Notes",
            question: "Add extra context.",
            isOther: true,
            isSecret: false,
            options: [
              {
                label: "Reuse current plan",
                description: "Keep the current plan structure.",
              },
            ],
          },
        ],
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
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    });

    const firstOption = container.querySelector(
      'input[data-request-user-input-option="Plan only"]'
    ) as HTMLInputElement | null;
    const otherOption = container.querySelector(
      'input[data-request-user-input-option="None of the above"]'
    ) as HTMLInputElement | null;
    const submitButton = container.querySelector(
      'button[data-request-user-input-submit="request-user-input:call-1"]'
    ) as HTMLButtonElement | null;

    expect(firstOption).not.toBeNull();
    expect(otherOption).not.toBeNull();
    expect(submitButton).not.toBeNull();
    if (!firstOption || !otherOption || !submitButton) return;

    act(() => {
      firstOption.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      otherOption.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    await act(async () => {
      submitButton.click();
    });

    expect(onSubmitRequestUserInput).toHaveBeenCalledTimes(1);
    expect(onSubmitRequestUserInput).toHaveBeenCalledWith(
      JSON.stringify(
        {
          scope: "Plan only",
          notes: "None of the above",
        },
        null,
        2
      )
    );
  });

  it("keeps in-progress request_user_input drafts across rerenders with equivalent questions", () => {
    const onSubmitRequestUserInput = () => {};
    const buildItems = (): ConversationItem[] => [
      {
        kind: "tool_call",
        id: "request-user-input:call-stable",
        title: "Question",
        status: "pending",
        raw_input: [
          {
            id: "notes",
            header: "Notes",
            question: "Add extra context.",
            isOther: false,
            isSecret: false,
          },
        ],
      },
    ];

    act(() => {
      root.render(
        <AcpConversation
          items={buildItems()}
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
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    });

    const textarea = container.querySelector(
      'textarea[data-request-user-input-note="notes"]'
    ) as HTMLTextAreaElement | null;
    expect(textarea).not.toBeNull();
    if (!textarea) return;

    act(() => {
      setNativeValue(textarea, "Keep this draft");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new Event("change", { bubbles: true }));
    });

    expect(textarea.value).toBe("Keep this draft");

    act(() => {
      root.render(
        <AcpConversation
          items={buildItems()}
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
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    });

    const rerenderedTextarea = container.querySelector(
      'textarea[data-request-user-input-note="notes"]'
    ) as HTMLTextAreaElement | null;
    expect(rerenderedTextarea).not.toBeNull();
    expect(rerenderedTextarea?.value).toBe("Keep this draft");
  });

  it("re-enables request_user_input submission controls after a successful submit callback", async () => {
    let resolveSubmission: (() => void) | null = null;
    const onSubmitRequestUserInput = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveSubmission = resolve;
        })
    );
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "request-user-input:call-submit-reset",
        title: "Question",
        status: "pending",
        raw_input: [
          {
            id: "notes",
            header: "Notes",
            question: "Add extra context.",
            isOther: false,
            isSecret: false,
          },
        ],
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
          onSubmitRequestUserInput={onSubmitRequestUserInput}
        />
      );
    });

    const textarea = container.querySelector(
      'textarea[data-request-user-input-note="notes"]'
    ) as HTMLTextAreaElement | null;
    const submitButton = container.querySelector(
      'button[data-request-user-input-submit="request-user-input:call-submit-reset"]'
    ) as HTMLButtonElement | null;

    expect(textarea).not.toBeNull();
    expect(submitButton).not.toBeNull();
    if (!textarea || !submitButton) return;

    act(() => {
      setNativeValue(textarea, "Keep trying");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new Event("change", { bubbles: true }));
    });

    await act(async () => {
      submitButton.click();
    });

    expect(onSubmitRequestUserInput).toHaveBeenCalledTimes(1);
    expect(submitButton.disabled).toBe(true);
    expect(submitButton.textContent).toContain("Submitting");

    await act(async () => {
      resolveSubmission?.();
      await Promise.resolve();
    });

    expect(submitButton.disabled).toBe(false);
    expect(submitButton.textContent).toContain("Submit Answer");
    expect(textarea.disabled).toBe(false);
  });
});
