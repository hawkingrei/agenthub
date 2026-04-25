// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  AcpPanel,
  AcpPanelProps,
  AcpPanelView,
  resolveAcpInputDockConversationClearance,
} from "./components/acp_panel";
import { AcpView } from "./acp";
import * as acpDebugLoader from "./components/acp_debug_loader";
import { installReactDomTestGlobals, renderWithMantine, required } from "./test_utils/react_test_helpers";

installReactDomTestGlobals();

const baseView: AcpView = {
  hasAcp: true,
  toolCalls: [],
  messages: [],
  rawEvents: [],
  configOptions: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

const baseProps: AcpPanelProps = {
  acpView: baseView,
  subtitle: null,
  acpTab: "conversation",
  developerMode: true,
  onSelectTab: () => {},
  showConversationBadge: false,
  showConversationJump: false,
  onJumpToConversationBottom: () => {},
  conversation: {
    items: [],
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
    containerRef: React.createRef<HTMLDivElement>() as React.RefObject<HTMLDivElement>,
    ansi: (input) => input,
  },
  plan: {
    plan: null,
  },
  debug: {
    terminalOutputs: [],
    ansi: (input) => input,
    terminalRef: React.createRef<HTMLDivElement>() as React.RefObject<HTMLDivElement>,
    onTerminalScroll: () => {},
    showTerminalJump: false,
    onJumpToTerminalBottom: () => {},
    currentMode: null,
    rawEvents: [],
    configOptions: [],
    acpPermissionHistory: [],
    acpModeId: "",
    acpModelId: "",
    acpConfigId: "",
    acpConfigValue: "",
    onAcpModeIdChange: () => {},
    onAcpModelIdChange: () => {},
    onAcpConfigIdChange: () => {},
    onAcpConfigValueChange: () => {},
    canControlAcp: false,
    onAcpSetMode: () => {},
    onAcpSetModel: () => {},
    onAcpSetConfig: () => {},
    onAcpCancel: () => {},
    onAcpClearSession: () => {},
    onJumpToPermissionHistory: () => {},
    runtimeMetrics: {
      totalConversationItems: 0,
      sourceConversationItems: 0,
      renderedConversationItems: 0,
      pendingConversationItems: 0,
      virtualizedConversation: false,
      stickToBottom: true,
      averageConversationHeight: 48,
      rawEventCount: 0,
      toolCallCount: 0,
      messageCount: 0,
      markdownCacheHits: 0,
      markdownCacheMisses: 0,
      ansiCacheHits: 0,
      ansiCacheMisses: 0,
      payloadParses: 0,
      payloadParseFailures: 0,
    },
  },
};

function renderPanel(node: React.ReactNode): Promise<string> {
  return Promise.resolve(
    renderToStaticMarkup(<MantineProvider>{node}</MantineProvider>).split("<!-- -->").join("")
  );
}

type ClickableButtonElement = React.ReactElement<{
  children?: React.ReactNode;
  className?: string;
  onClick?: () => void;
  type?: string;
}>;

function collectButtons(
  node: React.ReactNode,
  out: ClickableButtonElement[] = []
): ClickableButtonElement[] {
  if (node == null || typeof node === "string" || typeof node === "number") {
    return out;
  }
  if (Array.isArray(node)) {
    for (const child of node) collectButtons(child, out);
    return out;
  }
  if (!React.isValidElement(node)) return out;
  const element = node as ClickableButtonElement;
  if (
    (element.type === "button" ||
      (element.props.type === "button" && typeof element.props.onClick === "function")) &&
    typeof element.props.className === "string"
  ) {
    out.push(element);
  }
  collectButtons(element.props.children, out);
  return out;
}

describe("AcpPanel layout", () => {
  it("renders subtitle and tabs in header", async () => {
    const html = await renderPanel(
      <AcpPanel {...baseProps} subtitle="/repo/workdir" />
    );
    expect(html).toContain("/repo/workdir");
    expect(html).toContain("Activity");
    expect(html).toContain("Plan");
    expect(html).toContain("Inspect");
    expect(html).not.toContain("Interrupt");
  });

  it("hides debug tab and falls back to conversation when developer mode is off", async () => {
    const html = await renderPanel(
      <AcpPanel {...baseProps} developerMode={false} acpTab="debug" />
    );
    expect(html).toContain("Activity");
    expect(html).toContain("Plan");
    expect(html).not.toContain("Inspect");
    expect(html).not.toContain("Session Controls");
  });

  it("shows pending badge when conversation badge is enabled", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        showConversationBadge={true}
        conversation={{ ...baseProps.conversation, pendingCount: 3 }}
      />
    );
    expect(html).toContain("+3");
  });

  it("pads the conversation scroll region above a visible input dock", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        conversationBottomClearance={168}
      />
    );
    expect(html).toContain("scroll-padding-bottom:168px");
    expect(html).toContain('data-acp-conversation-scroll="true"');
  });

  it("renders a loading skeleton instead of partial conversation content while ACP history is still warming up", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        conversationLoading={true}
      />
    );
    expect(html).toContain('data-acp-conversation-loading-skeleton="true"');
    expect(html).not.toContain('data-acp-conversation-scroll="true"');
  });

  it("renders mobile title inline with tabs when provided", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        subtitle="/repo/workdir"
        mobileTitle="agenthub"
      />
    );
    expect(html).toContain("agenthub");
    expect(html).toContain("Activity");
    expect(html).toContain("Plan");
    expect(html).toContain("sm:hidden");
    expect(html).not.toContain("acp-actions");
  });

  it("invokes tab selection callbacks for both tabs", async () => {
    const onSelectTab = vi.fn();
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);

    try {
      renderWithMantine(
        root,
        <AcpPanel
          {...baseProps}
          onSelectTab={onSelectTab}
          showConversationBadge={true}
          conversation={{ ...baseProps.conversation, pendingCount: 1 }}
        />
      );

      const conversationButton = required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Activity")
        ) as HTMLButtonElement | undefined,
        "activity tab button missing"
      );
      const planButton = required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Plan")
        ) as HTMLButtonElement | undefined,
        "plan tab button missing"
      );
      const debugButton = required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Inspect")
        ) as HTMLButtonElement | undefined,
        "inspect tab button missing"
      );

      act(() => {
        conversationButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        planButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        debugButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      });
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }

    expect(onSelectTab).toHaveBeenNthCalledWith(1, "conversation");
    expect(onSelectTab).toHaveBeenNthCalledWith(2, "plan");
    expect(onSelectTab).toHaveBeenNthCalledWith(3, "debug");
  });

  it("renders plan view when acpTab is plan", async () => {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);

    try {
      renderWithMantine(
        root,
        <AcpPanel
          {...baseProps}
          acpTab="plan"
          plan={{
            plan: {
              entries: [
                { content: "Analyze issue", status: "completed", priority: "high" },
                { content: "Apply patch", status: "in_progress" },
              ],
            },
          }}
        />
      );

      await act(async () => {
        await vi.dynamicImportSettled();
      });

      expect(container.textContent).toContain("Current Plan");
      expect(container.textContent).toContain("Analyze issue");
      expect(container.textContent).toContain("Apply patch");
    } finally {
      act(() => {
        root.unmount();
      });
      container.remove();
    }
  });

  it("shows plan status on the tab when a plan exists", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        plan={{
          plan: {
            entries: [
              { content: "Analyze issue", status: "completed", priority: "high" },
              { content: "Apply patch", status: "in_progress" },
              { content: "Verify result" },
            ],
          },
        }}
      />
    );

    expect(html).toContain("Plan");
    expect(html).toContain("1 active");
  });

  it("keeps a minimum bottom clearance even before the dock reports its height", async () => {
    expect(resolveAcpInputDockConversationClearance(0)).toBe(64);
    expect(resolveAcpInputDockConversationClearance(156)).toBe(164);
  });

  it("shows done state on the plan tab when all entries are completed", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        plan={{
          plan: {
            entries: [
              { content: "Analyze issue", status: "completed" },
              { content: "Apply patch", status: "finished" },
            ],
          },
        }}
      />
    );

    expect(html).toContain("Plan");
    expect(html).toContain("done");
  });

  it("renders debug controls when acpTab is debug", async () => {
    const html = await renderPanel(
      <AcpPanel {...baseProps} acpTab="debug" />
    );
    expect(html).toContain("Loading debug...");
  });

  it("renders conversation jump button on ACP panel container layer", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        showConversationJump={true}
      />
    );
    const panelPos = html.indexOf('class="acp acp-panel');
    const conversationPos = html.indexOf('class="acp-conversation');
    const jumpPos = html.indexOf('aria-label="Jump to bottom"');
    expect(panelPos).toBeGreaterThanOrEqual(0);
    expect(conversationPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThanOrEqual(0);
    expect(jumpPos).toBeGreaterThan(conversationPos);
    expect(html).not.toContain("acp-conversation-jump-bottom");
  });

  it("passes bottom clearance into the conversation scroll area when input dock is present", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        conversationBottomClearance={104}
      />
    );
    expect(html).toContain('scroll-padding-bottom:104px');
    expect(html).not.toContain('class="acp-conversation-spacer dock-clearance"');
  });

  it("hides conversation jump button when debug tab is active", async () => {
    const html = await renderPanel(
      <AcpPanel
        {...baseProps}
        acpTab="debug"
        showConversationJump={true}
      />
    );
    expect(html).not.toContain("acp-jump-bottom");
  });

  it("invokes jump callback when ACP jump button is clicked", async () => {
    const onJumpToConversationBottom = vi.fn();
    const tree = AcpPanelView({
      ...baseProps,
      showConversationJump: true,
      onJumpToConversationBottom,
    });
    const buttons = collectButtons(tree);
    const jumpButton = buttons.find(
      (button) =>
        typeof button.props.className === "string" &&
        button.props.className.includes("acp-jump-bottom")
    );
    expect(jumpButton).toBeDefined();
    jumpButton?.props.onClick?.();
    expect(onJumpToConversationBottom).toHaveBeenCalledTimes(1);
  });
});

describe("AcpPanel debug loading fallback", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    container?.remove();
    vi.restoreAllMocks();
  });

  it("shows retry affordance and retries lazy debug loading after a failure", async () => {
    const loaderSpy = vi.spyOn(acpDebugLoader, "loadAcpDebugModule");
    loaderSpy
      .mockRejectedValueOnce(new Error("chunk missing"))
      .mockResolvedValueOnce({
        AcpDebug: () => <div>Debug ready</div>,
      } as never);
    const consoleErrorSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => undefined);

    renderWithMantine(root, <AcpPanel {...baseProps} acpTab="debug" />);

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.textContent).toContain(
      "Inspect panel failed to load. Try reloading this view."
    );
    const retryButton = required(
      Array.from(container.querySelectorAll("button")).find((button) =>
        button.textContent?.includes("Retry")
      ) as HTMLButtonElement | undefined,
      "retry button missing"
    );
    expect(consoleErrorSpy).toHaveBeenCalled();

    act(() => {
      retryButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.textContent).toContain("Debug ready");
    expect(loaderSpy).toHaveBeenCalledTimes(2);
  });
});
