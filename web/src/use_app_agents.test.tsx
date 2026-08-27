// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthState } from "./types";
import { useAppAgents } from "./use_app_agents";

const {
  listAgentsMock,
  listAgentNodesMock,
  listTeamsMock,
  getAgentMock,
  getAgentNodeJoinBootstrapMock,
  getRuntimeDefaultsMock,
  createAgentMock,
  setAgentCodexAcpDefaultModeMock,
  deleteAgentNodeMock,
} = vi.hoisted(() => ({
  listAgentsMock: vi.fn(),
  listAgentNodesMock: vi.fn(),
  listTeamsMock: vi.fn(),
  getAgentMock: vi.fn(),
  getAgentNodeJoinBootstrapMock: vi.fn(),
  getRuntimeDefaultsMock: vi.fn(),
  createAgentMock: vi.fn(),
  setAgentCodexAcpDefaultModeMock: vi.fn(),
  deleteAgentNodeMock: vi.fn(),
}));

vi.mock("./api", () => ({
  api: {
    listAgents: listAgentsMock,
    listAgentNodes: listAgentNodesMock,
    listTeams: listTeamsMock,
    getAgent: getAgentMock,
    getAgentNodeJoinBootstrap: getAgentNodeJoinBootstrapMock,
    getRuntimeDefaults: getRuntimeDefaultsMock,
    createAgent: createAgentMock,
    setAgentCodexAcpDefaultMode: setAgentCodexAcpDefaultModeMock,
    deleteAgentNode: deleteAgentNodeMock,
  },
  parseApiErrorMessage: vi.fn(() => null),
  stringifyApiError: vi.fn((error: unknown) => String(error)),
}));

type UseAppAgentsResult = ReturnType<typeof useAppAgents>;
type HookProps = Parameters<typeof useAppAgents>;

function HookHarness({
  auth,
  isAgentsRoute,
  agentStatusSseConnected = false,
  onCapture,
}: {
  auth: HookProps[0];
  isAgentsRoute: HookProps[1];
  agentStatusSseConnected?: HookProps[2];
  onCapture: (value: UseAppAgentsResult) => void;
}) {
  const value = useAppAgents(auth, isAgentsRoute, agentStatusSseConnected);
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
    listTeamsMock.mockReset();
    getAgentMock.mockReset();
    getAgentNodeJoinBootstrapMock.mockReset();
    getRuntimeDefaultsMock.mockReset();
    createAgentMock.mockReset();
    setAgentCodexAcpDefaultModeMock.mockReset();
    deleteAgentNodeMock.mockReset();
    listAgentsMock.mockResolvedValue([]);
    listAgentNodesMock.mockResolvedValue([]);
    listTeamsMock.mockResolvedValue([]);
    getAgentNodeJoinBootstrapMock.mockResolvedValue({
      enabled: true,
      bootstrap_token: "bootstrap-token",
      grpc_listen_addr: "0.0.0.0:50051",
      security_mode: "tls",
      cert_dir: "/etc/agenthub/internal-grpc",
      issuer: "agenthub",
      audience: "agenthub-internal",
    });
    getRuntimeDefaultsMock.mockResolvedValue({
      default_worktree_root: "~/.agenthub/worktrees",
    });
    createAgentMock.mockResolvedValue({
      id: "agent-created",
      name: "Created",
      workdir: "/tmp/created",
      command: "agenthub-codex-acp",
      args: [],
      target_node_id: null,
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: true,
      codex_acp_default_mode: "full-access",
      status: "created",
      created_at: 1,
      updated_at: 1,
    });
    setAgentCodexAcpDefaultModeMock.mockResolvedValue({ status: "ok" });
  });

  afterEach(() => {
    vi.useRealTimers();
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
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

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
      latest.setCodexAcpDefaultMode("auto");
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
    expect(latest.codexAcpDefaultMode).toBe("full-access");
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
    expect(latest.agentNodeJoinBootstrap).toBeNull();
    expect(latest.agentNodeJoinBootstrapLoading).toBe(false);
    expect(latest.agentNodeJoinBootstrapError).toBeNull();
  });

  it("creates codex agents with the selected startup permission mode", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      latest.setAgentName("codex-agent");
      latest.setAgentWorkdir("/tmp/codex-agent");
      latest.setCodexAcpDefaultMode("yolo");
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onCreateAgent();
      await Promise.resolve();
    });

    expect(createAgentMock).toHaveBeenCalledWith(
      "token-1",
      expect.objectContaining({
        name: "codex-agent",
        command: "agenthubd",
        args: ["acp", "codex"],
        codex_acp_default_mode: "full-access",
      })
    );
  });

  it("updates a codex agent startup mode without touching the running session", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      latest.setAgents([
        {
          id: "agent-codex",
          name: "Codex",
          workdir: "/tmp/codex",
          command: "agenthub-codex-acp",
          args: [],
          target_node_id: null,
          worktree_mode: "use_existing",
          worktree_repo: null,
          worktree_ref: null,
          code_mode: true,
          codex_acp_default_mode: "auto",
          status: "created",
          created_at: 1,
          updated_at: 1,
        },
      ]);
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onSetCodexAcpDefaultMode("agent-codex", "yolo");
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(setAgentCodexAcpDefaultModeMock).toHaveBeenCalledWith(
      "token-1",
      "agent-codex",
      "full-access"
    );
    expect(latest.agents[0]?.codex_acp_default_mode).toBe("full-access");
  });

  it("falls back to main when the selected target node no longer exists", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

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

  it("loads root-only node bootstrap details on the agents route", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    expect(getAgentNodeJoinBootstrapMock).toHaveBeenCalledWith("token-1");
    expect(latest.agentNodeJoinBootstrap).toEqual({
      enabled: true,
      bootstrap_token: "bootstrap-token",
      grpc_listen_addr: "0.0.0.0:50051",
      security_mode: "tls",
      cert_dir: "/etc/agenthub/internal-grpc",
      issuer: "agenthub",
      audience: "agenthub-internal",
    });
    expect(latest.agentNodeJoinBootstrapLoading).toBe(false);
    expect(latest.agentNodeJoinBootstrapError).toBeNull();
  });

  it("stores a named bootstrap error when the loader fails", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };
    getAgentNodeJoinBootstrapMock.mockRejectedValueOnce(new Error("boom"));

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    expect(latest.agentNodeJoinBootstrap).toBeNull();
    expect(latest.agentNodeJoinBootstrapLoading).toBe(false);
    expect(latest.agentNodeJoinBootstrapError).toBe(
      "Agent Node Join Bootstrap: Error: boom"
    );
  });

  it("keeps fallback agent polling while app-level SSE is disconnected", async () => {
    vi.useFakeTimers();
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={vi.fn()} />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(listAgentsMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(10_000);
      await Promise.resolve();
    });

    expect(listAgentsMock).toHaveBeenCalledTimes(2);
  });

  it("disables fallback agent polling while app-level SSE is connected", async () => {
    vi.useFakeTimers();
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAgentsRoute={true}
          agentStatusSseConnected={true}
          onCapture={vi.fn()}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(listAgentsMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(30_000);
      await Promise.resolve();
    });

    expect(listAgentsMock).toHaveBeenCalledTimes(1);
  });

  it("backfills hidden team member agents for node usage surfaces", async () => {
    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };
    listTeamsMock.mockResolvedValueOnce([
      {
        id: "team-1",
        name: "tidb fuzz/bugfix team",
        description: null,
        spec: {
          members: [{ member_id: "hidden-worker", role: "worker" }],
        },
        created_at: 1,
        updated_at: 1,
      },
    ]);
    getAgentMock.mockResolvedValueOnce({
      id: "hidden-worker",
      name: "tidb-fuzz-bugfix-team-worker-1",
      workdir: "/tmp/hidden-worker",
      command: "agenthub",
      args: [],
      target_node_id: "main",
      worktree_mode: "use_existing",
      worktree_repo: null,
      worktree_ref: null,
      code_mode: false,
      status: "idle",
      created_at: 1,
      updated_at: 1,
    });

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    expect(listTeamsMock).toHaveBeenCalledWith("token-1");
    expect(getAgentMock).toHaveBeenCalledWith("token-1", "hidden-worker");
    expect(latest.teamMemberAgentsById["hidden-worker"]?.target_node_id).toBe("main");
  });

  it("deletes a node only after the removal is confirmed", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    listAgentNodesMock.mockResolvedValue([
      {
        id: "node-east",
        name: "East",
        grpc_target: "grpc://east",
        tls_server_name: "east.local",
        default_worktree_root: "/tmp/east",
        last_seen_at: null,
        is_main: false,
        created_at: 1,
        updated_at: 1,
      },
    ]);
    deleteAgentNodeMock.mockResolvedValue(undefined);

    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      await latest.refreshAgentNodes();
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.agentNodes.map((node) => node.id)).toEqual(["node-east"]);

    await act(async () => {
      await latest.onDeleteAgentNode("node-east");
      await Promise.resolve();
    });

    expect(confirmSpy).toHaveBeenCalledWith('Remove node "East"?');
    expect(deleteAgentNodeMock).toHaveBeenCalledWith("token-1", "node-east");
    latest = captures[captures.length - 1];
    expect(latest.agentNodes.map((node) => node.id)).toEqual([]);

    confirmSpy.mockRestore();
  });

  it("does not delete a node when the confirmation is declined", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    listAgentNodesMock.mockResolvedValue([
      {
        id: "node-east",
        name: "East",
        grpc_target: "grpc://east",
        tls_server_name: "east.local",
        default_worktree_root: "/tmp/east",
        last_seen_at: null,
        is_main: false,
        created_at: 1,
        updated_at: 1,
      },
    ]);

    const captures: UseAppAgentsResult[] = [];
    const onCapture = (value: UseAppAgentsResult) => {
      captures.push(value);
    };
    const auth: AuthState = {
      token: "token-1",
      userId: "user-1",
      username: "root",
      role: "root",
    };

    await act(async () => {
      root.render(<HookHarness auth={auth} isAgentsRoute={true} onCapture={onCapture} />);
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      await latest.refreshAgentNodes();
      await Promise.resolve();
    });

    latest = captures[captures.length - 1];
    expect(latest.agentNodes.map((node) => node.id)).toEqual(["node-east"]);

    await act(async () => {
      await latest.onDeleteAgentNode("node-east");
      await Promise.resolve();
    });

    expect(deleteAgentNodeMock).not.toHaveBeenCalled();
    expect(captures[captures.length - 1].agentNodes.map((node) => node.id)).toEqual([
      "node-east",
    ]);

    confirmSpy.mockRestore();
  });
});
