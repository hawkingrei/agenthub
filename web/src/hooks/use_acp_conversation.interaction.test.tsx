// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AcpView } from "../acp";
import { useAcpConversation } from "./use_acp_conversation";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookSnapshot = ReturnType<typeof useAcpConversation>;

const baseAcpView: AcpView = {
  hasAcp: true,
  toolCalls: [
    {
      id: "call-1",
      title: "Read file",
      status: "completed",
      event_id: 1,
      seq: "1",
      session_id: "session-1",
    },
  ],
  messages: [],
  rawEvents: [],
  plan: null,
  commands: [],
  currentMode: null,
  runStatus: null,
  thinkingStartTs: null,
};

function buildManyMessageView(count: number): AcpView {
  return {
    ...baseAcpView,
    messages: Array.from({ length: count }, (_, idx) => ({
      kind: "agent_message" as const,
      text: `message-${idx}`,
      session_id: "session-1",
      event_id: idx + 1,
      seq: String(idx + 1),
      chunk: false,
    })),
  };
}

function HookHarness({
  acpView = baseAcpView,
  acpTab = "conversation",
  renderToolCallNode = true,
  onSnapshot,
}: {
  acpView?: AcpView;
  acpTab?: "conversation" | "plan" | "debug";
  renderToolCallNode?: boolean;
  onSnapshot: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useAcpConversation({
    acpView,
    activeAgent: "agent-1",
    activeSessionId: "session-1",
    acpTab,
    eventMeta: {},
    isAgentActive: true,
    onLoadOlder: () => {},
  });
  onSnapshot(snapshot);
  if (acpTab !== "conversation") {
    return <div data-mode={acpTab} />;
  }
  return (
    <div data-testid="conversation-viewport" ref={snapshot.acpConversationRef}>
      {snapshot.conversationRenderItems.map((item, idx) => (
        item.kind === "tool_call_group" ? (
          <div key={`${item.kind}-${idx}`}>
            {item.calls.map((call) => (
              <div
                key={call.id}
                data-tool-call-id={renderToolCallNode ? call.id : undefined}
              />
            ))}
          </div>
        ) : (
          <div
            key={`${item.kind}-${idx}`}
            data-tool-call-id={
              item.kind === "tool_call" && renderToolCallNode ? item.id : undefined
            }
          />
        )
      ))}
    </div>
  );
}

describe("useAcpConversation jump interaction", () => {
  let container: HTMLDivElement;
  let root: Root;
  let snapshot: HookSnapshot | null = null;
  const onSnapshot = (next: HookSnapshot) => {
    snapshot = next;
  };

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    snapshot = null;
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("jumps to tool call bubble and clears focus highlight after timeout", () => {
    vi.useFakeTimers();
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} />);
    });

    let result = false;
    act(() => {
      result = snapshot?.jumpToConversationToolCall("call-1") ?? false;
    });
    expect(result).toBe(true);
    expect(snapshot?.focusedConversationToolCallId).toBe("call-1");

    act(() => {
      vi.advanceTimersByTime(2500);
    });
    expect(snapshot?.focusedConversationToolCallId).toBeNull();
  });

  it("returns false when target tool call is missing", () => {
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} />);
    });

    let result = true;
    act(() => {
      result = snapshot?.jumpToConversationToolCall("missing-call") ?? true;
    });
    expect(result).toBe(false);
  });

  it("returns false when tool call exists but target bubble is not mounted", () => {
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} renderToolCallNode={false} />);
    });

    let result = true;
    act(() => {
      result = snapshot?.jumpToConversationToolCall("call-1") ?? true;
    });
    expect(result).toBe(false);
    expect(snapshot?.focusedConversationToolCallId).toBe("call-1");
  });

  it("jumps to grouped tool call bubble when target call is nested in a group", () => {
    const groupedAcpView: AcpView = {
      ...baseAcpView,
      toolCalls: [
        {
          id: "call-1",
          title: "Read file",
          status: "completed",
          event_id: 1,
          seq: "1",
          session_id: "session-1",
        },
        {
          id: "call-2",
          title: "Write file",
          status: "completed",
          event_id: 2,
          seq: "2",
          session_id: "session-1",
        },
      ],
    };
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} acpView={groupedAcpView} />);
    });

    let result = false;
    act(() => {
      result = snapshot?.jumpToConversationToolCall("call-2") ?? false;
    });

    expect(result).toBe(true);
    expect(snapshot?.focusedConversationToolCallId).toBe("call-2");
  });

  it("clears previous focus timer when re-jumping and resets focus at bottom jump", () => {
    vi.useFakeTimers();
    const clearSpy = vi.spyOn(window, "clearTimeout");
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} />);
    });

    act(() => {
      snapshot?.jumpToConversationToolCall("call-1");
    });
    expect(snapshot?.focusedConversationToolCallId).toBe("call-1");

    act(() => {
      snapshot?.jumpToConversationToolCall("call-1");
    });
    expect(clearSpy).toHaveBeenCalled();

    act(() => {
      snapshot?.jumpToConversationBottom();
    });
    expect(snapshot?.focusedConversationToolCallId).toBeNull();
  });
});

describe("useAcpConversation viewport observer lifecycle", () => {
  let container: HTMLDivElement;
  let root: Root;
  let observeSpy: ReturnType<typeof vi.fn>;
  let disconnectSpy: ReturnType<typeof vi.fn>;
  let resizeCallback: ResizeObserverCallback | null = null;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    observeSpy = vi.fn();
    disconnectSpy = vi.fn();
    resizeCallback = null;
    class ResizeObserverMock {
      constructor(callback: ResizeObserverCallback) {
        resizeCallback = callback;
      }
      observe = observeSpy;
      disconnect = disconnectSpy;
      unobserve = vi.fn();
    }
    vi.stubGlobal("ResizeObserver", ResizeObserverMock);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it("re-attaches the viewport observer when switching back to the conversation tab", () => {
    act(() => {
      root.render(<HookHarness onSnapshot={() => {}} acpTab="conversation" />);
    });
    expect(observeSpy).toHaveBeenCalledTimes(1);
    expect(resizeCallback).not.toBeNull();

    act(() => {
      root.render(<HookHarness onSnapshot={() => {}} acpTab="plan" />);
    });
    expect(disconnectSpy).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(<HookHarness onSnapshot={() => {}} acpTab="conversation" />);
    });
    expect(observeSpy).toHaveBeenCalledTimes(2);
  });
});

describe("useAcpConversation viewport width initialization", () => {
  let container: HTMLDivElement;
  let root: Root;
  let snapshot: HookSnapshot | null = null;
  let scrollTop = 0;
  let originalClientWidth: PropertyDescriptor | undefined;
  let originalClientHeight: PropertyDescriptor | undefined;
  let originalScrollHeight: PropertyDescriptor | undefined;
  let originalScrollTop: PropertyDescriptor | undefined;
  let originalRequestAnimationFrame: typeof window.requestAnimationFrame | undefined;

  const onSnapshot = (next: HookSnapshot) => {
    snapshot = next;
  };

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    snapshot = null;
    scrollTop = 0;
    originalRequestAnimationFrame = window.requestAnimationFrame;
    Reflect.deleteProperty(window, "requestAnimationFrame");
    originalClientWidth = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      "clientWidth"
    );
    originalClientHeight = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      "clientHeight"
    );
    originalScrollHeight = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      "scrollHeight"
    );
    originalScrollTop = Object.getOwnPropertyDescriptor(
      HTMLElement.prototype,
      "scrollTop"
    );
    Object.defineProperty(HTMLElement.prototype, "clientWidth", {
      configurable: true,
      get: () => 640,
    });
    Object.defineProperty(HTMLElement.prototype, "clientHeight", {
      configurable: true,
      get: () => 420,
    });
    Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
      configurable: true,
      get: () => 12_000,
    });
    Object.defineProperty(HTMLElement.prototype, "scrollTop", {
      configurable: true,
      get: () => scrollTop,
      set: (value: number) => {
        scrollTop = value;
      },
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    snapshot = null;
    if (originalRequestAnimationFrame) {
      Object.defineProperty(window, "requestAnimationFrame", {
        configurable: true,
        value: originalRequestAnimationFrame,
      });
    }
    restoreDescriptor(HTMLElement.prototype, "clientWidth", originalClientWidth);
    restoreDescriptor(HTMLElement.prototype, "clientHeight", originalClientHeight);
    restoreDescriptor(HTMLElement.prototype, "scrollHeight", originalScrollHeight);
    restoreDescriptor(HTMLElement.prototype, "scrollTop", originalScrollTop);
  });

  it("keeps virtualization active when switching back to conversation while scrolled up", () => {
    const acpView = buildManyMessageView(220);
    act(() => {
      root.render(<HookHarness onSnapshot={onSnapshot} acpView={acpView} />);
    });

    scrollTop = 11_580;
    act(() => {
      snapshot?.handleConversationScroll();
    });

    scrollTop = 0;
    act(() => {
      snapshot?.handleConversationScroll();
    });

    expect(snapshot?.conversationVirtualized).toBe(true);
    expect(snapshot?.conversationRenderedItems ?? 0).toBeLessThan(
      snapshot?.conversationSourceItems ?? 0
    );
    expect(snapshot?.conversationVirtualBottomSpacer ?? 0).toBeGreaterThan(0);

    act(() => {
      root.render(
        <HookHarness onSnapshot={onSnapshot} acpView={acpView} acpTab="plan" />
      );
    });
    act(() => {
      root.render(
        <HookHarness
          onSnapshot={onSnapshot}
          acpView={acpView}
          acpTab="conversation"
        />
      );
    });

    expect(snapshot?.conversationVirtualized).toBe(true);
    expect(snapshot?.conversationRenderedItems ?? 0).toBeLessThan(
      snapshot?.conversationSourceItems ?? 0
    );
    expect(snapshot?.conversationVirtualBottomSpacer ?? 0).toBeGreaterThan(0);
  });
});

function restoreDescriptor(
  target: object,
  key: PropertyKey,
  descriptor: PropertyDescriptor | undefined
): void {
  if (descriptor) {
    Object.defineProperty(target, key, descriptor);
    return;
  }
  Reflect.deleteProperty(target, key);
}
