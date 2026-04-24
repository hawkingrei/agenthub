// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AcpConversation } from "./components/acp_conversation";
import { ConversationItem } from "./conversation";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "./test_utils/react_test_helpers";

installReactDomTestGlobals();

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

function renderConversation(
  root: Root,
  props: React.ComponentProps<typeof AcpConversation>
): Promise<void> {
  renderWithMantine(root, <AcpConversation {...props} />);
  return flushDeferredConversationRender();
}

async function flushDeferredConversationRender(iterations = 4): Promise<void> {
  for (let idx = 0; idx < iterations; idx += 1) {
    await act(async () => {
      await vi.dynamicImportSettled();
      await Promise.resolve();
    });
  }
}

async function waitForElement<T extends Element>(
  resolve: () => T | null,
  message: string,
  attempts = 10
): Promise<T> {
  for (let idx = 0; idx < attempts; idx += 1) {
    const candidate = resolve();
    if (candidate) {
      return candidate;
    }
    await flushDeferredConversationRender();
  }
  throw new Error(message);
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

  it("collapses input/output subfolds when parent tool call fold is collapsed", async () => {
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

    await renderConversation(root, {
      items,
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
      ansi: (input) => input,
    });

    const toolFold = await waitForElement(
      () => container.querySelector(".acp-tool-fold") as HTMLDetailsElement | null,
      "tool fold not found"
    );

    const findSubfold = (label: string): HTMLDetailsElement => {
      const wrapper = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
        const firstSpan = node.querySelector("summary span");
        return firstSpan?.textContent?.trim() === label;
      }) as HTMLDivElement | undefined;
      const fold = wrapper?.querySelector("details");
      if (!(fold instanceof HTMLDetailsElement)) {
        throw new Error(`subfold not found: ${label}`);
      }
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

  it("shows newest lines first for long payloads and reveals older lines after Show more", async () => {
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

    await renderConversation(root, {
      items,
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
      ansi: (input) => input,
    });

    const contentFoldWrapper = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
      const firstSpan = node.querySelector("summary span");
      return firstSpan?.textContent?.trim() === "Content";
    }) as HTMLDivElement | undefined;
    const contentFold = await waitForElement(
      () => contentFoldWrapper?.querySelector("details") as HTMLDetailsElement | null,
      "content fold not found"
    );
    setDetailsOpen(contentFold, true);

    const beforePre = container.querySelector("pre.acp-content.acp-payload-text");
    expect(beforePre).not.toBeNull();
    expect(beforePre?.className).toContain("acp-terminal-pre");
    const preTextBeforeExpand = beforePre?.textContent ?? "";
    expect(preTextBeforeExpand).toContain("line-259");
    expect(preTextBeforeExpand).not.toContain("line-0");

    const showMoreButton = await waitForElement(
      () =>
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Show more")
        ) ?? null,
      "show more button not found"
    );

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

  it("renders markdown content folds with terminal tone", async () => {
    const items: ConversationItem[] = [
      {
        kind: "tool_call",
        id: "call-terminal-markdown",
        title: "Shell",
        status: "completed",
        content: "## Heading\n\n`tidb-server`\n\n- line one",
      },
    ];

    await renderConversation(root, {
      items,
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
      ansi: (input) => input,
    });

    const contentNode = await waitForElement(
      () => container.querySelector(".acp-content-markdown") as HTMLDivElement | null,
      "markdown content fold not found"
    );
    expect(contentNode?.textContent).toContain("Heading");
    expect(contentNode?.textContent).toContain("tidb-server");
  });

  it("keeps Detailed collapsed by default after output fold disappears on rerender", async () => {
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

    await renderConversation(root, { items: firstItems, ...baseProps });

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

    await renderConversation(root, { items: secondItems, ...baseProps });

    const detailedFoldWrapper = Array.from(container.querySelectorAll(".acp-subfold")).find((node) => {
      const firstSpan = node.querySelector("summary span");
      return firstSpan?.textContent?.trim() === "Detailed";
    }) as HTMLDivElement | undefined;
    const detailedFold = await waitForElement(
      () => detailedFoldWrapper?.querySelector("details") as HTMLDetailsElement | null,
      "detailed fold not found"
    );
    expect(detailedFold.open).toBe(false);
  });

  it("auto-collapses an older live tool call when it crosses the conversation cutoff", async () => {
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

    await renderConversation(root, {
      items,
      shouldAutoCollapse: false,
      collapseCutoff: 0,
      ...baseProps,
    });

    const toolFold = await waitForElement(
      () => container.querySelector(".acp-tool-fold") as HTMLDetailsElement | null,
      "tool fold not found on initial render"
    );
    expect(toolFold.open).toBe(true);

    await renderConversation(root, {
      items,
      shouldAutoCollapse: true,
      collapseCutoff: 10,
      ...baseProps,
    });

    const collapsedToolFold = await waitForElement(
      () => container.querySelector(".acp-tool-fold") as HTMLDetailsElement | null,
      "tool fold not found after rerender"
    );
    expect(collapsedToolFold.open).toBe(false);
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

    await renderConversation(root, {
      items,
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
      ansi: (input) => input,
      onSubmitRequestUserInput,
    });

    const firstOption = await waitForElement(
      () =>
        container.querySelector(
          'input[data-request-user-input-option="Plan only"]'
        ) as HTMLInputElement | null,
      "first request_user_input option not found"
    );
    const otherOption = await waitForElement(
      () =>
        container.querySelector(
          'input[data-request-user-input-option="None of the above"]'
        ) as HTMLInputElement | null,
      "other request_user_input option not found"
    );
    const submitButton = await waitForElement(
      () =>
        container.querySelector(
          'button[data-request-user-input-submit="request-user-input:call-1"]'
        ) as HTMLButtonElement | null,
      "request_user_input submit button not found"
    );

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

  it("keeps in-progress request_user_input drafts across rerenders with equivalent questions", async () => {
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

    await renderConversation(root, {
      items: buildItems(),
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
      ansi: (input) => input,
      onSubmitRequestUserInput,
    });

    const textarea = await waitForElement(
      () =>
        container.querySelector(
          'textarea[data-request-user-input-note="notes"]'
        ) as HTMLTextAreaElement | null,
      "request_user_input notes textarea not found"
    );

    act(() => {
      setNativeValue(textarea, "Keep this draft");
      textarea.dispatchEvent(new Event("input", { bubbles: true }));
      textarea.dispatchEvent(new Event("change", { bubbles: true }));
    });

    expect(textarea.value).toBe("Keep this draft");

    await renderConversation(root, {
      items: buildItems(),
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
      ansi: (input) => input,
      onSubmitRequestUserInput,
    });

    const rerenderedTextarea = await waitForElement(
      () =>
        container.querySelector(
          'textarea[data-request-user-input-note="notes"]'
        ) as HTMLTextAreaElement | null,
      "rerendered request_user_input notes textarea not found"
    );
    expect(rerenderedTextarea.value).toBe("Keep this draft");
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

    await renderConversation(root, {
      items,
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
      ansi: (input) => input,
      onSubmitRequestUserInput,
    });

    const textarea = await waitForElement(
      () =>
        container.querySelector(
          'textarea[data-request-user-input-note="notes"]'
        ) as HTMLTextAreaElement | null,
      "request_user_input notes textarea not found"
    );
    const submitButton = await waitForElement(
      () =>
        container.querySelector(
          'button[data-request-user-input-submit="request-user-input:call-submit-reset"]'
        ) as HTMLButtonElement | null,
      "request_user_input submit button not found"
    );

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
