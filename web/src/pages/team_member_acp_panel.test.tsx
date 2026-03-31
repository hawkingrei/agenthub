// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import type { AgentEvent } from "../api";
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { useAcpConversation } from "../hooks/use_acp_conversation";

vi.mock("../hooks/use_acp_conversation", () => ({
  useAcpConversation: vi.fn(),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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

if (typeof globalThis.ResizeObserver !== "function") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  } as typeof ResizeObserver;
}

function required<T>(value: T | null | undefined, message: string): T {
  if (value == null) {
    throw new Error(message);
  }
  return value;
}

function renderWithMantine(root: Root, node: React.ReactNode): void {
  act(() => {
    root.render(<MantineProvider>{node}</MantineProvider>);
  });
}

function buildAcpEvents(): AgentEvent[] {
  return [
    {
      event_id: 1,
      agent_id: "worker-agent",
      session_id: "runtime-session-1",
      seq: "1",
      ts: 1_700_000_001,
      stream: "acp",
      message: JSON.stringify({
        type: "agent_message",
        text: "Runtime conversation is active.",
      }),
    },
  ];
}

function buildConversationHookState(overrides: Record<string, unknown> = {}) {
  return {
    acpConversationRef: React.createRef<HTMLDivElement>(),
    collapseCutoff: 80,
    conversationAvgHeight: 72,
    conversationPendingCount: 0,
    conversationRenderItems: [],
    conversationSourceItems: 0,
    conversationRenderedItems: 0,
    conversationTotalItems: 0,
    conversationVirtualBottomSpacer: 0,
    conversationVirtualTopSpacer: 0,
    conversationWindowOffset: 0,
    conversationStickToBottom: false,
    conversationVirtualized: false,
    focusedConversationToolCallId: null,
    handleConversationScroll: vi.fn(),
    isFrozenView: false,
    jumpToConversationBottom: vi.fn(),
    shouldAutoCollapse: false,
    showConversationBadge: false,
    showConversationJump: true,
    showConversationTopReachedHint: false,
    ...overrides,
  };
}

describe("TeamMemberAcpPanel jump-to-bottom alignment", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.mocked(useAcpConversation).mockReturnValue(buildConversationHookState() as never);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
  });

  it("falls back to the floating ACP jump when the team member input dock is unavailable", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onRefresh={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.querySelector(".jump-bottom")).toBeNull();
    expect(container.querySelector(".acp-jump-bottom")).not.toBeNull();
  });

  it("keeps the dock jump and hides the floating ACP jump when input is available", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onRefresh={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.querySelector(".jump-bottom")).not.toBeNull();
    expect(container.querySelector(".acp-jump-bottom")).toBeNull();
    expect(required(container.querySelector("textarea"), "input dock textarea missing")).toBeTruthy();
  });
});
