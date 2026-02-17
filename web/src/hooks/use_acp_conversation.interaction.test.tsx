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

function HookHarness({
  acpView = baseAcpView,
  acpTab = "conversation",
  renderToolCallNode = true,
  onSnapshot,
}: {
  acpView?: AcpView;
  acpTab?: "conversation" | "debug";
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
  return (
    <div ref={snapshot.acpConversationRef}>
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
