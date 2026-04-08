// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppAgents } from "./use_app_agents";

const {
  listAgentsMock,
  listAgentNodesMock,
  getRuntimeDefaultsMock,
} = vi.hoisted(() => ({
  listAgentsMock: vi.fn(),
  listAgentNodesMock: vi.fn(),
  getRuntimeDefaultsMock: vi.fn(),
}));

vi.mock("./api", () => ({
  api: {
    listAgents: listAgentsMock,
    listAgentNodes: listAgentNodesMock,
    getRuntimeDefaults: getRuntimeDefaultsMock,
  },
  parseApiErrorMessage: vi.fn(() => null),
}));

type UseAppAgentsResult = ReturnType<typeof useAppAgents>;
type HookProps = Parameters<typeof useAppAgents>;

function HookHarness({
  auth,
  isAgentsRoute,
  onCapture,
}: {
  auth: HookProps[0];
  isAgentsRoute: HookProps[1];
  onCapture: (value: UseAppAgentsResult) => void;
}) {
  const value = useAppAgents(auth, isAgentsRoute);
  useEffect(() => {
    onCapture(value);
  }, [onCapture, value]);
  return null;
}

describe("useAppAgents", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    listAgentsMock.mockReset();
    listAgentNodesMock.mockReset();
    getRuntimeDefaultsMock.mockReset();
    listAgentsMock.mockResolvedValue([]);
    listAgentNodesMock.mockResolvedValue([]);
    getRuntimeDefaultsMock.mockResolvedValue({
      default_worktree_root: "~/.agenthub/worktrees",
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
  });

  it("resets draft and node state when token becomes null", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth = { token: "token-1", rootInitialized: true } as HookProps[0];

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    expect(latest).toBeDefined();

    await act(async () => {
      latest.setNodeIdInput("node-east");
      latest.setNodeNameInput("East");
      latest.setNodeGrpcTargetInput("grpc://east");
      latest.setNodeTlsServerNameInput("east.local");
      latest.setNodeDefaultWorktreeRootInput("/tmp/east");
      latest.setShowCreateAgent(true);
      latest.setAgentName("agent-east");
      latest.setCodeMode(false);
      latest.setWorktreeRepo("git@example.com/repo.git");
      latest.setWorktreeRef("main");
      latest.setStartingAgentIds({ "agent-1": true });
      await Promise.resolve();
    });

    await act(async () => {
      root.render(<HookHarness auth={null} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.showCreateAgent).toBe(false);
    expect(latest.agentName).toBe("");
    expect(latest.codeMode).toBe(true);
    expect(latest.worktreeRepo).toBe("");
    expect(latest.worktreeRef).toBe("");
    expect(latest.nodeIdInput).toBe("");
    expect(latest.nodeNameInput).toBe("");
    expect(latest.nodeGrpcTargetInput).toBe("");
    expect(latest.nodeTlsServerNameInput).toBe("");
    expect(latest.nodeDefaultWorktreeRootInput).toBe("");
    expect(latest.startingAgentIds).toEqual({});
    expect(latest.targetNodeId).toBe("main");
    expect(latest.agents).toEqual([]);
    expect(latest.agentNodes).toEqual([]);
  });

  it("falls back to main when the selected target node no longer exists", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth = { token: "token-1", role: "root", rootInitialized: true } as HookProps[0];

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    expect(latest).toBeDefined();

    const nodes = [
      {
        id: "node-east",
        name: "East",
        grpc_target: "grpc://east",
        tls_server_name: null,
        default_worktree_root: "/tmp/east",
      },
    ];

    listAgentNodesMock.mockResolvedValueOnce(nodes);
    await act(async () => {
      await latest.refreshAgentNodes();
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.agentNodes).toEqual(nodes);

    await act(async () => {
      latest.applyTargetNodeSelection("node-east");
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.targetNodeId).toBe("node-east");

    listAgentNodesMock.mockResolvedValueOnce([]);
    await act(async () => {
      await latest.refreshAgentNodes();
      await Promise.resolve();
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.agentNodes).toEqual([]);
    expect(latest.targetNodeId).toBe("main");
  });
});
