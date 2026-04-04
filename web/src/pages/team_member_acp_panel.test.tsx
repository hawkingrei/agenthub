// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot } from "react-dom/client";
import type { Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AgentEvent } from "../api";
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import {
  installReactDomTestGlobals,
  renderWithMantine,
  required,
} from "../test_utils/react_test_helpers";

vi.mock("../hooks/use_acp_conversation", () => ({
  useAcpConversation: vi.fn(),
}));

installReactDomTestGlobals();

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
      stream: "acp",
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

    expect(container.querySelector(".acp-jump-bottom")).not.toBeNull();
    expect(required(container.querySelector("textarea"), "input dock textarea missing")).toBeTruthy();
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
        onRefresh={vi.fn()}
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

  it("submits selected ACP mode and model values for the team member", () => {
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
        onRefresh={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    act(() => {
      required(
        Array.from(container.querySelectorAll("button")).find((button) =>
          button.textContent?.includes("Debug")
        ) as HTMLButtonElement | undefined,
        "debug tab button missing"
      ).dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });

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
});
