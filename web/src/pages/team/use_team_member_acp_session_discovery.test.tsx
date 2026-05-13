// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS,
  useTeamMemberAcpSessionDiscovery,
} from "./use_team_member_acp_session_discovery";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamMemberAcpSessionDiscovery>[0];

function HookHarness({ params }: { params: HookParams }) {
  useTeamMemberAcpSessionDiscovery(params);
  return null;
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    activeRunId: "run-1",
    tab: "agent_acp",
    selectedMemberId: "worker-1",
    selectedSessionId: null,
    snapshotStatus: "working",
    agentStatus: "running",
    runtimeSessionStatus: null,
    runtimeAgentStatus: null,
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe("useTeamMemberAcpSessionDiscovery", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("refreshes immediately and then polls while an active ACP member has no session id", async () => {
    const refreshSnapshot = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ refreshSnapshot });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(refreshSnapshot).toHaveBeenCalledTimes(1);
    expect(refreshSnapshot).toHaveBeenLastCalledWith("run-1");

    await act(async () => {
      vi.advanceTimersByTime(TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(refreshSnapshot).toHaveBeenCalledTimes(2);
    expect(refreshSnapshot).toHaveBeenLastCalledWith("run-1");
  });

  it("stops polling once a selected session id appears", async () => {
    const refreshSnapshot = vi.fn().mockResolvedValue(undefined);
    const params = createParams({ refreshSnapshot });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(refreshSnapshot).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <HookHarness
          params={{
            ...params,
            selectedSessionId: "session-1",
          }}
        />
      );
    });
    await act(async () => {
      vi.advanceTimersByTime(TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS * 2);
      await Promise.resolve();
    });

    expect(refreshSnapshot).toHaveBeenCalledTimes(1);
  });

  it("does not refresh outside ACP session tabs or for stopped runtime state", async () => {
    const refreshSnapshot = vi.fn().mockResolvedValue(undefined);

    act(() => {
      root.render(
        <HookHarness
          params={createParams({
            refreshSnapshot,
            tab: "mailbox",
          })}
        />
      );
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(refreshSnapshot).not.toHaveBeenCalled();

    act(() => {
      root.render(
        <HookHarness
          params={createParams({
            refreshSnapshot,
            snapshotStatus: "stopped",
            agentStatus: "stopped",
          })}
        />
      );
    });
    await act(async () => {
      vi.advanceTimersByTime(TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(refreshSnapshot).not.toHaveBeenCalled();
  });
});
