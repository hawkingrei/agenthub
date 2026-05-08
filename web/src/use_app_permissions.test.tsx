// @vitest-environment jsdom
import { act, useMemo, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AcpPermissionRecord, type AgentRecord } from "./api";
import {
  GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS,
  GLOBAL_PERMISSION_POLL_INTERVAL_MS,
} from "./app_permission_polling";
import {
  type AcpPermissionLiveSignal,
  useAppPermissions,
} from "./use_app_permissions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function buildAgent(id: string): AgentRecord {
  return {
    id,
    name: id,
    workdir: `/tmp/${id}`,
    command: "codex",
    args: [],
    target_node_id: null,
    worktree_mode: "use_existing",
    worktree_repo: null,
    worktree_ref: null,
    code_mode: true,
    agent_loop_enabled: undefined,
    agent_loop_idle_seconds: null,
    agent_loop_prompt: null,
    status: "running",
    created_at: 1,
    updated_at: 1,
  };
}

function buildPermission(id: string, agentId: string): AcpPermissionRecord {
  return {
    id,
    agent_id: agentId,
    session_id: `${agentId}-session`,
    options: [],
    status: "pending",
    created_at: 1,
  };
}

type HookProps = {
  permissionSseConnected: boolean;
  permissionLiveSignal: AcpPermissionLiveSignal;
  activeAgent?: string | null;
  developerMode?: boolean;
  acpTab?: string;
};

const agents = [buildAgent("agent-a"), buildAgent("agent-b")];

function HookHarness({
  permissionSseConnected,
  permissionLiveSignal,
  activeAgent = "agent-a",
  developerMode = true,
  acpTab = "debug",
}: HookProps) {
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>([]);
  const [pendingPermissionCounts, setPendingPermissionCounts] = useState<Record<string, number>>({});
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<AcpPermissionRecord[]>([]);
  const permissionState = useMemo(
    () => ({
      acpPermissions,
      setAcpPermissions,
      pendingPermissionCounts,
      setPendingPermissionCounts,
      acpPermissionHistory,
      setAcpPermissionHistory,
    }),
    [acpPermissions, pendingPermissionCounts, acpPermissionHistory]
  );

  useAppPermissions(
    { token: "token-1", role: "admin", userId: "user-1", username: "user-1" },
    true,
    agents,
    activeAgent,
    false,
    developerMode,
    acpTab,
    permissionSseConnected,
    permissionLiveSignal,
    permissionState
  );
  return null;
}

describe("useAppPermissions", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(api, "listAcpPermissions").mockImplementation(async (_token, agentId, status) => {
      if (status === "pending") {
        return [buildPermission(`pending-${agentId}`, agentId)];
      }
      return [buildPermission(`history-${agentId}`, agentId)];
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("uses initial loads and live signals instead of interval polling while SSE is connected", async () => {
    await act(async () => {
      root.render(
        <HookHarness
          permissionSseConnected
          permissionLiveSignal={{ seq: 0, agentIds: [] }}
        />
      );
      await Promise.resolve();
      vi.advanceTimersByTime(0);
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(3);

    await act(async () => {
      vi.advanceTimersByTime(GLOBAL_PERMISSION_POLL_INTERVAL_MS + GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS);
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(3);

    await act(async () => {
      root.render(
        <HookHarness
          permissionSseConnected
          permissionLiveSignal={{ seq: 1, agentIds: ["agent-a", "agent-b"] }}
        />
      );
      await Promise.resolve();
      vi.advanceTimersByTime(0);
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledWith("token-1", "agent-a", "pending");
    expect(api.listAcpPermissions).toHaveBeenCalledWith("token-1", "agent-b", "pending");
    expect(api.listAcpPermissions).toHaveBeenCalledWith("token-1", "agent-a");
    expect(api.listAcpPermissions).toHaveBeenCalledTimes(7);
  });

  it("keeps interval polling as the fallback when SSE is not connected", async () => {
    await act(async () => {
      root.render(
        <HookHarness
          permissionSseConnected={false}
          permissionLiveSignal={{ seq: 0, agentIds: [] }}
        />
      );
      await Promise.resolve();
    });
    await act(async () => {
      vi.advanceTimersByTime(0);
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(3);

    await act(async () => {
      vi.advanceTimersByTime(GLOBAL_PERMISSION_POLL_ACTIVE_DELAY_MS);
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(4);
  });

  it("refreshes only signaled inactive-agent counts from live permission events", async () => {
    await act(async () => {
      root.render(
        <HookHarness
          permissionSseConnected
          permissionLiveSignal={{ seq: 0, agentIds: [] }}
        />
      );
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(3);

    await act(async () => {
      root.render(
        <HookHarness
          permissionSseConnected
          permissionLiveSignal={{ seq: 1, agentIds: ["agent-b", "missing-agent"] }}
        />
      );
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(4);
    expect(api.listAcpPermissions).toHaveBeenLastCalledWith("token-1", "agent-b", "pending");
  });

  it("does not load debug history from a live signal outside the debug tab", async () => {
    await act(async () => {
      root.render(
        <HookHarness
          acpTab="conversation"
          permissionSseConnected
          permissionLiveSignal={{ seq: 0, agentIds: [] }}
        />
      );
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(2);

    await act(async () => {
      root.render(
        <HookHarness
          acpTab="conversation"
          permissionSseConnected
          permissionLiveSignal={{ seq: 1, agentIds: ["agent-a"] }}
        />
      );
      await Promise.resolve();
    });

    expect(api.listAcpPermissions).toHaveBeenCalledTimes(4);
    expect(api.listAcpPermissions).not.toHaveBeenCalledWith("token-1", "agent-a");
  });
});
