// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentEvent } from "../api";
import * as acpModule from "../acp";
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import {
  clearTeamMemberAcpRenderCache,
  saveTeamMemberAcpRenderCache,
} from "./team/team_member_acp_render_cache";
import {
  isActiveTeamMemberStatus,
  normalizeTeamMemberStatusValue,
} from "./team/use_team_member_acp_view_model";
import {
  installReactDomTestGlobals,
  renderWithMantine,
  required,
} from "../test_utils/react_test_helpers";

vi.mock("../hooks/use_acp_conversation", () => ({
  useAcpConversation: vi.fn(),
}));

installReactDomTestGlobals();

async function openDebugTabAndWait(container: HTMLElement): Promise<void> {
  act(() => {
    required(
      Array.from(container.querySelectorAll("button")).find((button) =>
        button.textContent?.includes("Inspect")
      ) as HTMLButtonElement | undefined,
      "inspect tab button missing"
    ).dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });

  await act(async () => {
    await vi.dynamicImportSettled();
  });
}

function buildAcpEvents(extraMessages: Record<string, unknown>[] = []): AgentEvent[] {
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
    ...extraMessages.map((message, index) => ({
      event_id: index + 2,
      agent_id: "worker-agent",
      session_id: "runtime-session-1",
      seq: String(index + 2),
      ts: 1_700_000_001 + index + 1,
      stream: "acp" as const,
      message: JSON.stringify(message),
    })),
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
    handleConversationWheel: vi.fn(),
    isFrozenView: false,
    jumpToConversationBottom: vi.fn(),
    shouldAutoCollapse: false,
    showConversationBadge: false,
    showConversationJump: true,
    showConversationTopReachedHint: false,
    ...overrides,
  };
}

function latestAcpConversationArgs() {
  const calls = vi.mocked(useAcpConversation).mock.calls;
  return required(calls[calls.length - 1]?.[0], "ACP conversation args missing");
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
    clearTeamMemberAcpRenderCache();
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
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.querySelector(".jump-bottom")).toBeNull();
    expect(container.querySelector(".acp-jump-bottom")).not.toBeNull();
  });

  it("keeps the dock jump and hides the floating ACP jump when input is available", async () => {
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
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.querySelector(".acp-jump-bottom")).not.toBeNull();
    expect(required(container.querySelector("textarea"), "input dock textarea missing")).toBeTruthy();
    const conversation = required(
      container.querySelector('[data-acp-conversation-scroll="true"]') as HTMLDivElement | null,
      "acp conversation missing"
    );
    expect(conversation.classList.contains("py-0.5")).toBe(true);
  });

  it("pads the ACP conversation above the measured input dock height", () => {
    let resizeCallback: ResizeObserverCallback | null = null;
    const originalResizeObserver = globalThis.ResizeObserver;
    const originalGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect;

    class MockResizeObserver {
      constructor(callback: ResizeObserverCallback) {
        resizeCallback = callback;
      }
      observe(): void {}
      unobserve(): void {}
      disconnect(): void {}
    }

    globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;
    HTMLElement.prototype.getBoundingClientRect = function getBoundingClientRect(): DOMRect {
      if ((this as HTMLElement).classList?.contains("input-dock-shell")) {
        return {
          x: 0,
          y: 0,
          width: 640,
          height: 156,
          top: 0,
          right: 640,
          bottom: 156,
          left: 0,
          toJSON: () => ({}),
        } as DOMRect;
      }
      return originalGetBoundingClientRect.call(this);
    };

    try {
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
          onLoadOlder={vi.fn()}
        />
      );

      act(() => {
        resizeCallback?.([], {} as ResizeObserver);
      });

      const conversation = required(
        container.querySelector('[data-acp-conversation-scroll="true"]') as HTMLDivElement | null,
        "acp conversation scroll container missing"
      );
      expect(conversation.style.paddingBottom).toBe("");
      expect(conversation.style.scrollPaddingBottom).toBe("164px");
    } finally {
      globalThis.ResizeObserver = originalResizeObserver;
      HTMLElement.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    }
  });

  it("shows an interrupt action for a running team member ACP session", () => {
    const onInterrupt = vi.fn();

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "running" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        canInterrupt={true}
        onInterrupt={onInterrupt}
        onLoadOlder={vi.fn()}
      />
    );

    const interruptButton = required(
      container.querySelector('button[aria-label="Interrupt current run"]'),
      "interrupt button missing"
    ) as HTMLButtonElement;
    expect(interruptButton.disabled).toBe(false);

    act(() => {
      interruptButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(onInterrupt).toHaveBeenCalledTimes(1);
  });

  it("shows an interrupt action for a live tool call without a running status", () => {
    const onInterrupt = vi.fn();

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([
          { type: "tool_call", id: "call-live", title: "Shell", status: "in_progress" },
        ])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        canInterrupt={true}
        onInterrupt={onInterrupt}
        onLoadOlder={vi.fn()}
      />
    );

    const interruptButton = required(
      container.querySelector('button[aria-label="Interrupt current run"]'),
      "interrupt button missing"
    ) as HTMLButtonElement;
    expect(interruptButton.disabled).toBe(false);
  });

  it("loads older history through the ACP conversation hook", () => {
    const onLoadOlder = vi.fn();

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={true}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={1}
        onLoadOlder={onLoadOlder}
      />
    );

    act(() => {
      latestAcpConversationArgs().onLoadOlder();
    });

    expect(onLoadOlder).toHaveBeenCalledTimes(1);
  });

  it("shows a compact ACP activity strip for loaded updates, tool calls, and older history", async () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 2,
        conversationRenderedItems: 2,
        conversationTotalItems: 2,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([
          {
            type: "tool_call_update",
            id: "tool-1",
            content: [
              {
                type: "content",
                content: { type: "text", text: "{\"command\": \"cargo test\"}" },
              },
            ],
          },
        ])}
        memberEventsHasMore={true}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={11}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await Promise.resolve();
    });

    const activityStrip = required(
      container.querySelector('[data-team-member-acp-activity-strip="true"]') as HTMLDivElement | null,
      "acp activity strip missing"
    );
    expect(activityStrip.textContent).toContain("2 updates");
    expect(activityStrip.textContent).toContain("1 tool call");
    expect(activityStrip.textContent).toContain("Older history available");
  });

  it("shows syncing in the ACP activity strip while refreshing a live session", async () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 1,
        conversationRenderedItems: 1,
        conversationTotalItems: 1,
      }) as never
    );

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
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await Promise.resolve();
    });

    const activityStrip = required(
      container.querySelector('[data-team-member-acp-activity-strip="true"]') as HTMLDivElement | null,
      "acp activity strip missing"
    );
    expect(activityStrip.textContent).toContain("1 update");
    expect(activityStrip.textContent).toContain("Syncing");
  });

  it("hides the ACP activity strip when no ACP session is available yet", async () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 3,
        conversationRenderedItems: 3,
        conversationTotalItems: 3,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId={undefined}
        selectedMemberRole="worker"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "pending",
          session_status: "pending",
        }}
        selectedAgentStatus="running"
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={true}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.querySelector('[data-team-member-acp-activity-strip="true"]')).toBeNull();
  });

  it("prefers the selected member snapshot status over stale ACP run status in the header", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "idle",
          session_status: "idle",
        }}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "running" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("idle");
    expect(container.textContent).toContain("Thread");
    expect(container.textContent).not.toContain("running · thinking");
  });

  it("uses explicit member titles when provided", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        memberTitle="Runtime debugger"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "idle",
          session_status: "idle",
        }}
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
      />
    );

    expect(container.textContent).toContain("Runtime debugger");
  });

  it("labels coordinator members with the coordinator default title", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="coordinator-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="coordinator"
        selectedMemberSnapshot={{
          member_id: "coordinator-agent",
          role: "coordinator",
          skills: [],
          pending_inbox_count: 0,
          status: "idle",
          session_status: "idle",
        }}
        memberEvents={buildAcpEvents()}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
      />
    );

    expect(container.textContent).toContain("Coordinator agent");
  });

  it("prefers an active member snapshot over a stale stopped agent record", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="stopped"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "idle",
          session_status: "idle",
        }}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "idle" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("idle");
    expect(container.textContent).toContain("Active thread");
    expect(latestAcpConversationArgs().activeSessionId).toBe("runtime-session-1");
    expect(latestAcpConversationArgs().isAgentActive).toBe(true);
    expect(container.textContent).not.toContain("Agent is stopped");
  });

  it("uses active session_status as a fallback for stale stopped agent records", async () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="stopped"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: " ",
          session_status: " Idle ",
        }}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "idle" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.textContent).toContain("idle");
    expect(container.textContent).toContain("Active thread");
    expect(required(container.querySelector("textarea"), "input dock textarea missing")).toBeTruthy();
    expect(latestAcpConversationArgs().activeSessionId).toBe("runtime-session-1");
    expect(latestAcpConversationArgs().isAgentActive).toBe(true);
    expect(container.textContent).not.toContain("Agent is stopped");
  });

  it("falls back to the latest step runtime handle when no session prop is selected", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId={undefined}
        selectedMemberRole="worker"
        selectedAgentStatus="running"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "running",
          session_status: "running",
          latest_step: {
            id: "step-1",
            run_id: "run-1",
            step_key: "analysis",
            member_id: "worker-agent",
            status: "working",
            attempt: 1,
            depends_on: [],
            runtime_handle_id: " runtime-session-from-step ",
            remote_task_id: null,
          },
        }}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "running" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(latestAcpConversationArgs().activeSessionId).toBe("runtime-session-from-step");
    expect(latestAcpConversationArgs().isAgentActive).toBe(true);
  });

  it("keeps stopped member snapshots from reusing stale ACP sessions", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="stopped"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "stopped",
          session_status: "stopped",
        }}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "idle" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("stopped");
    expect(container.textContent).toContain("Agent is stopped");
    expect(container.querySelector("textarea")).toBeNull();
    expect(latestAcpConversationArgs().activeSessionId).toBeNull();
    expect(latestAcpConversationArgs().isAgentActive).toBe(false);
  });

  it("uses stopped member snapshots to close stale live tool calls", async () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationRenderItems: [
          {
            kind: "tool_call",
            id: "call-stale",
            title: "Shell",
            status: "in_progress",
            raw_input: { cmd: "cargo test" },
          },
        ],
        conversationSourceItems: 1,
        conversationRenderedItems: 1,
        conversationTotalItems: 1,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="running"
        selectedMemberSnapshot={{
          member_id: "worker-agent",
          role: "worker",
          skills: [],
          pending_inbox_count: 0,
          status: "stopped",
          session_status: "stopped",
        }}
        memberEvents={buildAcpEvents([
          { type: "tool_call", id: "call-stale", title: "Shell", status: "in_progress" },
        ])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await act(async () => {
      await vi.dynamicImportSettled();
    });

    expect(container.textContent).toContain("Agent is stopped");
    expect(container.textContent).toContain("Stopped");
    expect(container.textContent).not.toContain("In Progress");
  });

  it("uses active agent status when no member snapshot is available", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="running"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "idle" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("idle");
    expect(container.textContent).toContain("Active thread");
    expect(required(container.querySelector("textarea"), "input dock textarea missing")).toBeTruthy();
    expect(latestAcpConversationArgs().activeSessionId).toBe("runtime-session-1");
    expect(latestAcpConversationArgs().isAgentActive).toBe(true);
  });

  it("blocks stale sessions when only the agent record is stopped", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedAgentStatus="stopped"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([{ type: "run_status", status: "idle" }])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("stopped");
    expect(container.textContent).toContain("Agent is stopped");
    expect(container.querySelector("textarea")).toBeNull();
    expect(latestAcpConversationArgs().activeSessionId).toBeNull();
    expect(latestAcpConversationArgs().isAgentActive).toBe(false);
  });

  it("shows startup state for active agents before a session is attached", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId={undefined}
        selectedMemberRole="worker"
        selectedAgentStatus="running"
        selectedMemberSnapshot={null}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Starting ACP session...");
    expect(container.textContent).toContain("Starting session");
    expect(container.querySelector("textarea")).toBeNull();
    expect(latestAcpConversationArgs().activeSessionId).toBeNull();
    expect(latestAcpConversationArgs().isAgentActive).toBe(false);
  });

  it("hides a partial leading ACP chunk instead of rendering it", () => {
    vi.mocked(useAcpConversation).mockReturnValue(buildConversationHookState() as never);

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[
          {
            event_id: 7,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "7",
            ts: 123,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "partial markdown",
              chunk: true,
              message_id: "msg-1",
              chunk_index: 12,
            }),
          },
        ]}
        memberEventsHasMore={true}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={7}
        onLoadOlder={vi.fn()}
      />
    );

    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).toBeNull();
    expect(container.textContent).toContain("Active thread");
    expect(container.textContent).not.toContain("Earlier reply truncated");
    const acpConversationCalls = vi.mocked(useAcpConversation).mock.calls;
    const latestCall = acpConversationCalls[acpConversationCalls.length - 1]?.[0];
    expect(latestCall?.acpView.messages).toEqual([]);
  });

  it("keeps visible ACP content on screen while background history refresh is still loading", () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationRenderItems: [
          {
            kind: "agent_message",
            text: "fully loaded content",
            event_id: 20,
            ts: 130,
          },
        ],
        conversationSourceItems: 1,
        conversationRenderedItems: 1,
        conversationTotalItems: 1,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[
          {
            event_id: 7,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "7",
            ts: 123,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "partial markdown",
              chunk: true,
              message_id: "msg-1",
              chunk_index: 12,
            }),
          },
        ]}
        memberEventsHasMore={true}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={7}
        onLoadOlder={vi.fn()}
      />
    );

    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).toBeNull();
    expect(container.textContent).toContain("Active thread");
  });

  it("uses warm session cache immediately after switching to a new ACP session", () => {
    clearTeamMemberAcpRenderCache();
    const cachedEvents = buildAcpEvents();
    saveTeamMemberAcpRenderCache("worker-agent", "runtime-session-1", cachedEvents);

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).not.toContain("Loading activity...");
    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).toBeNull();
    expect(useAcpConversation).toHaveBeenCalled();
    const acpConversationCalls = vi.mocked(useAcpConversation).mock.calls;
    const latestCall = acpConversationCalls[acpConversationCalls.length - 1]?.[0];
    expect(
      latestCall?.acpView.messages.some(
        (message: { text?: string }) => message.text === "Runtime conversation is active."
      )
    ).toBe(true);
  });

  it("does not render the leading incomplete ACP message while older history is still loading", () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 1,
        conversationRenderedItems: 1,
        conversationTotalItems: 1,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[
          {
            event_id: 7,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "7",
            ts: 123,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "partial markdown",
              chunk: true,
              message_id: "msg-1",
              chunk_index: 12,
            }),
          },
          {
            event_id: 20,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "20",
            ts: 130,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "fully loaded content",
              chunk: false,
              message_id: "msg-2",
            }),
          },
        ]}
        memberEventsHasMore={true}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={7}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).not.toContain("partial markdown");
    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).toBeNull();
    expect(container.textContent).toContain("Active thread");
  });

  it("uses activity-focused loading copy while the ACP thread is still initializing", () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 0,
        conversationRenderedItems: 0,
        conversationTotalItems: 0,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Loading activity...");
    expect(container.textContent).not.toContain("Restoring session...");
  });

  it("keeps showing loading copy when only non-ACP events are present during initialization", () => {
    vi.mocked(useAcpConversation).mockReturnValue(
      buildConversationHookState({
        conversationSourceItems: 0,
        conversationRenderedItems: 0,
        conversationTotalItems: 0,
      }) as never
    );

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[
          {
            event_id: 99,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "99",
            ts: 1_700_000_099,
            stream: "stdout",
            message: "plain terminal output",
          },
        ]}
        memberEventsHasMore={false}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Loading activity...");
    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).not.toBeNull();
  });

  it("reuses cached ACP events while the selected member session is refreshing", () => {
    vi.mocked(useAcpConversation).mockImplementation(
      ((args: { acpView: { messages: Array<{ text?: string | null }> } }) =>
        buildConversationHookState({
          conversationRenderItems: args.acpView.messages.map((message, index) => ({
            kind: "agent_message",
            text: message.text ?? "",
            event_id: index + 1,
            ts: 100 + index,
          })),
          conversationSourceItems: args.acpView.messages.length,
          conversationRenderedItems: args.acpView.messages.length,
          conversationTotalItems: args.acpView.messages.length,
        })) as never
    );

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
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Runtime conversation is active.");

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={true}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Runtime conversation is active.");
    expect(container.textContent).not.toContain("Loading activity...");
    expect(
      container.querySelector('[data-acp-conversation-loading-skeleton="true"]')
    ).toBeNull();
  });

  it("submits selected ACP mode and model values for the team member", async () => {
    const onAcpSetMode = vi.fn();
    const onAcpSetModel = vi.fn();

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedSessionId="runtime-session-1"
        selectedMemberRole="worker"
        selectedMemberSnapshot={null}
        memberEvents={buildAcpEvents([
          {
            type: "config_option_update",
            config_options: [
              {
                id: "mode",
                label: "Mode",
                current_value: { type: "value_id", value: "workspace_write" },
                select_options: [
                  { value_id: "workspace_write", label: "Workspace Write" },
                  { value_id: "danger_full_access", label: "Full Access" },
                ],
              },
              {
                id: "model",
                label: "Model",
                current_value: { type: "value_id", value: "gpt-5" },
                select_options: [
                  { value_id: "gpt-5", label: "GPT-5" },
                  { value_id: "gemini-2.5-pro", label: "Gemini 2.5 Pro" },
                ],
              },
            ],
          },
        ])}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        canControlAcp={true}
        onAcpSetMode={onAcpSetMode}
        onAcpSetModel={onAcpSetModel}
        onLoadOlder={vi.fn()}
      />
    );

    await openDebugTabAndWait(container);

    const modeSelect = required(
      container.querySelector('select[name="acp-mode"]') as HTMLSelectElement | null,
      "mode select missing"
    );
    const modelSelect = required(
      container.querySelector('select[name="acp-model"]') as HTMLSelectElement | null,
      "model select missing"
    );

    act(() => {
      modeSelect.value = "danger_full_access";
      modeSelect.dispatchEvent(new Event("change", { bubbles: true }));
      modelSelect.value = "gemini-2.5-pro";
      modelSelect.dispatchEvent(new Event("change", { bubbles: true }));
    });

    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Set Mode")
        ) as HTMLButtonElement | undefined,
        "set mode button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true }));
      required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Set Model")
        ) as HTMLButtonElement | undefined,
        "set model button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(onAcpSetMode).toHaveBeenCalledWith("danger_full_access");
    expect(onAcpSetModel).toHaveBeenCalledWith("gemini-2.5-pro");
  });

  it("does not rebuild ACP view when parent state changes without member ACP prop changes", () => {
    const buildAcpViewSpy = vi.spyOn(acpModule, "buildAcpView");
    const sharedEvents = buildAcpEvents();
    const panelProps = {
      developerMode: true,
      selectedMemberId: "worker-agent",
      selectedSessionId: "runtime-session-1",
      selectedMemberRole: "worker",
      selectedMemberSnapshot: null,
      memberEvents: sharedEvents,
      memberEventsHasMore: false,
      memberEventsLoading: false,
      eventsLoading: false,
      oldestMemberEventId: null,
      onSendInput: vi.fn(),
      onLoadOlder: vi.fn(),
    } as const;

    function Wrapper() {
      const [tick, setTick] = React.useState(0);
      return (
        <>
          <button type="button" onClick={() => setTick((current) => current + 1)}>
            bump {tick}
          </button>
          <TeamMemberAcpPanel {...panelProps} />
        </>
      );
    }

    renderWithMantine(root, <Wrapper />);
    const initialCallCount = buildAcpViewSpy.mock.calls.length;
    expect(initialCallCount).toBeGreaterThan(0);

    const bumpButton = required(
      Array.from(container.querySelectorAll("button")).find((candidate) =>
        candidate.textContent?.includes("bump")
      ) as HTMLButtonElement | undefined,
      "parent rerender button missing"
    );

    act(() => {
      bumpButton.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

    expect(buildAcpViewSpy).toHaveBeenCalledTimes(initialCallCount);
  });
});

describe("team member ACP status helpers", () => {
  it("normalizes nullish and padded status values", () => {
    expect(normalizeTeamMemberStatusValue(null)).toBe("");
    expect(normalizeTeamMemberStatusValue(undefined)).toBe("");
    expect(normalizeTeamMemberStatusValue(" Idle ")).toBe("idle");
  });

  it("treats idle runtime snapshots as active", () => {
    expect(isActiveTeamMemberStatus("idle")).toBe(true);
    expect(isActiveTeamMemberStatus("running")).toBe(true);
    expect(isActiveTeamMemberStatus("stopped")).toBe(false);
    expect(isActiveTeamMemberStatus(null)).toBe(false);
  });
});
