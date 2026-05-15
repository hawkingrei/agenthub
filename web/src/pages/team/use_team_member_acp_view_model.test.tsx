// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTeamMemberAcpViewModel } from "./use_team_member_acp_view_model";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamMemberAcpViewModel>[0];
type HookResult = ReturnType<typeof useTeamMemberAcpViewModel>;

function HookHarness({
  params,
  onCapture,
}: {
  params: HookParams;
  onCapture: (result: HookResult) => void;
}) {
  onCapture(useTeamMemberAcpViewModel(params));
  return null;
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    developerMode: false,
    selectedMemberId: "worker-1",
    memberTitle: "Worker",
    selectedMemberSnapshot: {
      member_id: "worker-1",
      role: "worker",
      model: "codex",
      description: null,
      prompt: null,
      skills: [],
      pending_inbox_count: 0,
      status: "idle",
      latest_step: null,
      session_status: null,
    },
    selectedMemberRole: "worker",
    selectedAgentStatus: "running",
    selectedSessionId: null,
    memberEvents: [],
    memberEventsHasMore: false,
    memberEventsLoading: false,
    oldestMemberEventId: null,
    acpTab: "conversation",
    inputDockHeight: 0,
    terminalShowJump: false,
    acpModeId: "",
    acpModelId: "",
    acpConfigId: "",
    acpConfigValue: "",
    canControlAcp: false,
    canInterrupt: false,
    setAcpTab: vi.fn(),
    setAcpModeId: vi.fn(),
    setAcpModelId: vi.fn(),
    setAcpConfigId: vi.fn(),
    setAcpConfigValue: vi.fn(),
    ansi: (input) => input,
    terminalRef: React.createRef<HTMLDivElement>(),
    handleTerminalScroll: vi.fn(),
    jumpToTerminalBottom: vi.fn(),
    handleSubmitRequestUserInput: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("useTeamMemberAcpViewModel", () => {
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
    vi.restoreAllMocks();
  });

  it("uses terminal runtime status instead of stale idle snapshot while no session is selected", () => {
    const onCapture = vi.fn();

    act(() => {
      root.render(
        <HookHarness
          params={createParams({ selectedAgentStatus: "exited" })}
          onCapture={onCapture}
        />
      );
    });

    const result = onCapture.mock.lastCall?.[0] as HookResult;
    expect(result.isStartingAcpSession).toBe(false);
    expect(result.panelSubtitle).toBe("Agent is stopped");
    expect(result.memberStatus).toBe("exited");
  });

  it("does not report starting while member session selection is loading", () => {
    const onCapture = vi.fn();

    act(() => {
      root.render(
        <HookHarness
          params={createParams({
            selectedMemberSnapshot: {
              member_id: "worker-2",
              role: "worker",
              model: "codex",
              description: null,
              prompt: null,
              skills: [],
              pending_inbox_count: 0,
              status: "running",
              latest_step: null,
              session_status: "running",
            },
            selectedMemberId: "worker-2",
            selectedSessionId: null,
            memberEventsLoading: true,
            selectedAgentStatus: "running",
          })}
          onCapture={onCapture}
        />
      );
    });

    const result = onCapture.mock.lastCall?.[0] as HookResult;
    expect(result.isStartingAcpSession).toBe(false);
    expect(result.panelSubtitle).toBe("Loading activity...");
    expect(result.memberStatus).toBe("running");
  });

  it("does not report starting only because an agent is running without a selected session", () => {
    const onCapture = vi.fn();

    act(() => {
      root.render(
        <HookHarness
          params={createParams({
            selectedMemberSnapshot: null,
            selectedAgentStatus: "running",
            selectedSessionId: null,
          })}
          onCapture={onCapture}
        />
      );
    });

    const result = onCapture.mock.lastCall?.[0] as HookResult;
    expect(result.isStartingAcpSession).toBe(false);
    expect(result.panelSubtitle).toBe("No active thread session yet");
    expect(result.memberStatus).toBe("running");
  });

});
