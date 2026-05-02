import { useState, useCallback, useRef, useEffect } from "react";
import {
  api,
  AgentNodeJoinBootstrapInfo,
  AgentRecord,
  AgentNodeRecord,
  TeamDefinitionRecord,
  stringifyApiError,
} from "./api";
import { AuthState } from "./types";
import { 
  canManageAgentNodes, 
  isSameAgentNodeRecordList, 
  isSameAgentRecordList, 
  removeAgentNodeRecord, 
  replaceAgentNodeRecord, 
  upsertAgentNodeRecord,
} from "./app_agents_helpers";
import {
  resolveDefaultWorktreeRootForTargetNode,
  resolveWorkdirForModeChange,
  resolveWorkdirForTargetNodeChange,
  resolveWorkdirForModalOpen,
  normalizeRuntimeWorktreeRoot,
  normalizeWorkdirInput
} from "./worktree_defaults";
import { formatWorktreeError } from "./app_utils";
import { validateAgentNodeDraft } from "./components/agent_node_validation";
import { useTeamMemberAgentBackfillEffect } from "./pages/team/use_team_member_agent_backfill_effect";
import { 
  DEFAULT_AGENT_PRESET_ID, 
  getAgentPreset, 
  type AgentPresetId 
} from "./agent_presets";

const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
const AGENT_STATUS_REFRESH_INTERVAL_MS = 10_000;

export function useAppAgents(auth: AuthState | null, isAgentsRoute: boolean) {
  const token = auth?.token ?? null;
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [agentNodes, setAgentNodes] = useState<AgentNodeRecord[]>([]);
  const [teams, setTeams] = useState<TeamDefinitionRecord[]>([]);
  const [teamMemberAgentsById, setTeamMemberAgentsById] = useState<
    Record<string, AgentRecord | null>
  >({});
  const [agentNodeJoinBootstrap, setAgentNodeJoinBootstrap] =
    useState<AgentNodeJoinBootstrapInfo | null>(null);
  const [agentNodeJoinBootstrapLoading, setAgentNodeJoinBootstrapLoading] =
    useState(false);
  const [agentNodeJoinBootstrapError, setAgentNodeJoinBootstrapError] =
    useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [worktreeError, setWorktreeError] = useState<string | null>(null);
  const [showCreateAgent, setShowCreateAgent] = useState(false);
  const [createAgentBusy, setCreateAgentBusy] = useState(false);
  const [startingAgentIds, setStartingAgentIds] = useState<Record<string, boolean>>({});

  const [agentName, setAgentName] = useState("");
  const [agentWorkdir, setAgentWorkdir] = useState("");
  const [defaultWorktreeRoot, setDefaultWorktreeRoot] = useState(DEFAULT_WORKTREE_ROOT);
  const [agentPresetId, setAgentPresetId] = useState<AgentPresetId>(DEFAULT_AGENT_PRESET_ID);
  const [worktreeMode, setWorktreeMode] = useState<"use_existing" | "create_worktree" | "reuse_worktree">("use_existing");
  const [worktreeRepo, setWorktreeRepo] = useState("");
  const [worktreeRef, setWorktreeRef] = useState("");
  const [codeMode, setCodeMode] = useState(true);
  const [targetNodeId, setTargetNodeId] = useState("main");

  const [nodeIdInput, setNodeIdInput] = useState("");
  const [nodeNameInput, setNodeNameInput] = useState("");
  const [nodeGrpcTargetInput, setNodeGrpcTargetInput] = useState("");
  const [nodeTlsServerNameInput, setNodeTlsServerNameInput] = useState("");
  const [nodeDefaultWorktreeRootInput, setNodeDefaultWorktreeRootInput] = useState("");
  const [createAgentNodeBusy, setCreateAgentNodeBusy] = useState(false);
  const [updatingAgentNodeIds, setUpdatingAgentNodeIds] = useState<Record<string, boolean>>({});
  const [deletingAgentNodeIds, setDeletingAgentNodeIds] = useState<Record<string, boolean>>({});

  const agentNodesRef = useRef(agentNodes);
  const selectedTargetNodeDefaultWorktreeRootRef = useRef("");

  useEffect(() => {
    agentNodesRef.current = agentNodes;
  }, [agentNodes]);

  const refreshAgents = useCallback(
    async (opts?: { silent?: boolean }): Promise<AgentRecord[] | null> => {
      if (!token) return null;
      const silent = opts?.silent === true;
      try {
        const items = await api.listAgents(token);
        setAgents((prev) => (isSameAgentRecordList(prev, items) ? prev : items));
        return items;
      } catch (err: unknown) {
        if (!silent) {
          setError(stringifyApiError(err));
        }
        return null;
      }
    },
    [token]
  );

  const refreshAgentNodes = useCallback(
    async (opts?: { silent?: boolean }): Promise<AgentNodeRecord[] | null> => {
      if (!token || !canManageAgentNodes(auth)) {
        setAgentNodes([]);
        return null;
      }
      const silent = opts?.silent === true;
      try {
        const items = await api.listAgentNodes(token);
        setAgentNodes((prev) => (isSameAgentNodeRecordList(prev, items) ? prev : items));
        return items;
      } catch (err: unknown) {
        if (!silent) {
          setError(stringifyApiError(err));
        }
        return null;
      }
    },
    [auth, token]
  );

  const refreshTeams = useCallback(
    async (opts?: { silent?: boolean }): Promise<TeamDefinitionRecord[] | null> => {
      if (!token || !canManageAgentNodes(auth)) {
        setTeams([]);
        return null;
      }
      const silent = opts?.silent === true;
      try {
        const items = await api.listTeams(token);
        setTeams(items);
        return items;
      } catch (err: unknown) {
        if (!silent) {
          setError(stringifyApiError(err));
        }
        return null;
      }
    },
    [auth, token]
  );

  const refreshAgentNodeJoinBootstrap = useCallback(
    async (
      opts?: { silent?: boolean }
    ): Promise<AgentNodeJoinBootstrapInfo | null> => {
      if (!token || !canManageAgentNodes(auth)) {
        setAgentNodeJoinBootstrap(null);
        setAgentNodeJoinBootstrapLoading(false);
        setAgentNodeJoinBootstrapError(null);
        return null;
      }
      const silent = opts?.silent === true;
      setAgentNodeJoinBootstrapLoading(true);
      setAgentNodeJoinBootstrapError(null);
      try {
        const info = await api.getAgentNodeJoinBootstrap(token);
        setAgentNodeJoinBootstrap(info);
        setAgentNodeJoinBootstrapError(null);
        return info;
      } catch (err: unknown) {
        const message =
          `Agent Node Join Bootstrap: ${stringifyApiError(err)}`;
        setAgentNodeJoinBootstrap(null);
        setAgentNodeJoinBootstrapError(message);
        if (!silent) {
          setError(message);
        }
        return null;
      } finally {
        setAgentNodeJoinBootstrapLoading(false);
      }
    },
    [auth, token]
  );

  useEffect(() => {
    if (!token || !isAgentsRoute) return;
    void refreshAgents();
  }, [isAgentsRoute, token, refreshAgents]);

  useEffect(() => {
    if (!token || !isAgentsRoute) return;
    void refreshAgentNodeJoinBootstrap({ silent: true });
  }, [isAgentsRoute, token, refreshAgentNodeJoinBootstrap]);

  useEffect(() => {
    if (!token || !isAgentsRoute) return;
    void refreshTeams({ silent: true });
  }, [isAgentsRoute, token, refreshTeams]);

  useEffect(() => {
    if (!token || !isAgentsRoute) return;
    const timer = window.setInterval(() => {
      void refreshAgents({ silent: true });
    }, AGENT_STATUS_REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [isAgentsRoute, token, refreshAgents]);

  useEffect(() => {
    if (!token) {
      setAgents([]);
      setAgentNodes([]);
      setTeams([]);
      setTeamMemberAgentsById({});
      setAgentNodeJoinBootstrap(null);
      setAgentNodeJoinBootstrapLoading(false);
      setAgentNodeJoinBootstrapError(null);
      setError(null);
      setWorktreeError(null);
      setStartingAgentIds({});
      setShowCreateAgent(false);
      setCreateAgentBusy(false);
      setAgentName("");
      setAgentPresetId(DEFAULT_AGENT_PRESET_ID);
      setWorktreeMode("use_existing");
      setWorktreeRepo("");
      setWorktreeRef("");
      setCodeMode(true);
      setTargetNodeId("main");
      setUpdatingAgentNodeIds({});
      setDeletingAgentNodeIds({});
      setNodeIdInput("");
      setNodeNameInput("");
      setNodeGrpcTargetInput("");
      setNodeTlsServerNameInput("");
      setNodeDefaultWorktreeRootInput("");
      setCreateAgentNodeBusy(false);
      setDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      setAgentWorkdir("");
      selectedTargetNodeDefaultWorktreeRootRef.current = "";
      return;
    }
    api
      .getRuntimeDefaults(token)
      .then((defaults) => {
        const root = normalizeRuntimeWorktreeRoot(
          defaults.default_worktree_root,
          DEFAULT_WORKTREE_ROOT
        );
        setDefaultWorktreeRoot(root);
        setAgentWorkdir((prev) =>
          resolveWorkdirForModeChange(
            prev,
            "use_existing",
            root,
            DEFAULT_WORKTREE_ROOT
          )
        );
      })
      .catch(() => undefined);
  }, [token]);

  const teamSpecMemberIds = Array.from(
    new Set(
      teams.flatMap((team) => {
        const members = (team.spec as { members?: unknown } | null)?.members;
        if (!Array.isArray(members)) {
          return [];
        }
        return members
          .map((member) =>
            typeof member === "object" &&
            member !== null &&
            typeof (member as { member_id?: unknown }).member_id === "string"
              ? (member as { member_id: string }).member_id.trim()
              : ""
          )
          .filter((memberId) => memberId.length > 0);
      })
    )
  );

  useTeamMemberAgentBackfillEffect({
    token: token ?? "",
    agents,
    teamSpecMemberIds,
    teamMemberAgentsById,
    setTeamMemberAgentsById,
  });

  const applyTargetNodeSelection = useCallback(
    (nextTargetNodeId: string, nextNodes: AgentNodeRecord[] = agentNodes) => {
      const requestedTargetNodeId = nextTargetNodeId.trim() || "main";
      const resolvedTargetNodeId =
        requestedTargetNodeId === "main" ||
        nextNodes.some((node) => node.id === requestedTargetNodeId)
          ? requestedTargetNodeId
          : "main";
      const nextDefaultRoot = resolveDefaultWorktreeRootForTargetNode(
        resolvedTargetNodeId,
        nextNodes,
        defaultWorktreeRoot,
        DEFAULT_WORKTREE_ROOT
      );
      setAgentWorkdir((prev) =>
        resolveWorkdirForTargetNodeChange(
          prev,
          worktreeMode,
          selectedTargetNodeDefaultWorktreeRootRef.current,
          nextDefaultRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
      selectedTargetNodeDefaultWorktreeRootRef.current = nextDefaultRoot;
      setTargetNodeId(resolvedTargetNodeId);
    },
    [agentNodes, defaultWorktreeRoot, worktreeMode]
  );

  useEffect(() => {
    const normalizedTargetNodeId = targetNodeId.trim() || "main";
    if (
      normalizedTargetNodeId !== "main" &&
      !agentNodes.some((node) => node.id === normalizedTargetNodeId)
    ) {
      applyTargetNodeSelection("main", agentNodes);
    }
  }, [agentNodes, applyTargetNodeSelection, targetNodeId]);

  const handleWorktreeModeChange = useCallback(
    (nextMode: "use_existing" | "create_worktree" | "reuse_worktree") => {
      setWorktreeMode(nextMode);
      setAgentWorkdir((prev) =>
        resolveWorkdirForModeChange(
          prev,
          nextMode,
          selectedTargetNodeDefaultWorktreeRootRef.current,
          DEFAULT_WORKTREE_ROOT
        )
      );
    },
    [selectedTargetNodeDefaultWorktreeRootRef]
  );

  const openCreateAgentModal = useCallback(() => {
    const mainDefaultRoot = resolveDefaultWorktreeRootForTargetNode(
      "main",
      agentNodes,
      defaultWorktreeRoot,
      DEFAULT_WORKTREE_ROOT
    );
    setAgentWorkdir((prev) =>
      resolveWorkdirForTargetNodeChange(
        resolveWorkdirForModalOpen(
          prev,
          worktreeMode,
          mainDefaultRoot,
          DEFAULT_WORKTREE_ROOT
        ),
        worktreeMode,
        selectedTargetNodeDefaultWorktreeRootRef.current,
        mainDefaultRoot,
        DEFAULT_WORKTREE_ROOT
      )
    );
    selectedTargetNodeDefaultWorktreeRootRef.current = mainDefaultRoot;
    setTargetNodeId("main");
    setShowCreateAgent(true);
    void refreshAgentNodes({ silent: true });
    void refreshAgentNodeJoinBootstrap({ silent: true });
  }, [
    agentNodes,
    defaultWorktreeRoot,
    refreshAgentNodeJoinBootstrap,
    refreshAgentNodes,
    worktreeMode,
  ]);

  const onCreateAgentNode = useCallback(async () => {
    if (!token || createAgentNodeBusy) return;
    const draftError = validateAgentNodeDraft({
      nodeId: nodeIdInput,
      nodeName: nodeNameInput,
      grpcTarget: nodeGrpcTargetInput,
    });
    if (draftError) {
      setError(draftError);
      return;
    }
    setError(null);
    setCreateAgentNodeBusy(true);
    try {
      const node = await api.createAgentNode(token, {
        id: nodeIdInput.trim(),
        name: nodeNameInput.trim(),
        grpc_target: nodeGrpcTargetInput.trim(),
        tls_server_name: nodeTlsServerNameInput.trim() || null,
        default_worktree_root: nodeDefaultWorktreeRootInput.trim() || null,
      });
      const nextNodes = upsertAgentNodeRecord(agentNodesRef.current, node);
      setAgentNodes(nextNodes);
      applyTargetNodeSelection(node.id, nextNodes);
      setNodeIdInput("");
      setNodeNameInput("");
      setNodeGrpcTargetInput("");
      setNodeTlsServerNameInput("");
      setNodeDefaultWorktreeRootInput("");
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    } finally {
      setCreateAgentNodeBusy(false);
    }
  }, [
    applyTargetNodeSelection,
    createAgentNodeBusy,
    nodeGrpcTargetInput,
    nodeDefaultWorktreeRootInput,
    nodeIdInput,
    nodeNameInput,
    nodeTlsServerNameInput,
    token,
  ]);

  const onUpdateAgentNode = useCallback(
    async (
      nodeId: string,
      payload: {
        name: string;
        grpc_target: string;
        tls_server_name?: string | null;
        default_worktree_root?: string | null;
      }
    ) => {
      if (!token || nodeId === "main") return;
      setError(null);
      setUpdatingAgentNodeIds((prev) => ({ ...prev, [nodeId]: true }));
      try {
        const node = await api.updateAgentNode(token, nodeId, payload);
        const nextNodes = replaceAgentNodeRecord(agentNodesRef.current, node);
        setAgentNodes(nextNodes);
        if (targetNodeId === nodeId) {
          applyTargetNodeSelection(nodeId, nextNodes);
        }
      } catch (err: unknown) {
        setError(stringifyApiError(err));
      } finally {
        setUpdatingAgentNodeIds((prev) => {
          if (!prev[nodeId]) return prev;
          const next = { ...prev };
          delete next[nodeId];
          return next;
        });
      }
    },
    [applyTargetNodeSelection, targetNodeId, token]
  );

  const onDeleteAgentNode = useCallback(
    async (nodeId: string) => {
      if (!token || nodeId === "main") return;
      setError(null);
      setDeletingAgentNodeIds((prev) => ({ ...prev, [nodeId]: true }));
      try {
        await api.deleteAgentNode(token, nodeId);
        const nextNodes = removeAgentNodeRecord(agentNodesRef.current, nodeId);
        setAgentNodes(nextNodes);
        if (targetNodeId === nodeId) {
          applyTargetNodeSelection("main", nextNodes);
        }
      } catch (err: unknown) {
        setError(stringifyApiError(err));
      } finally {
        setDeletingAgentNodeIds((prev) => {
          if (!prev[nodeId]) return prev;
          const next = { ...prev };
          delete next[nodeId];
          return next;
        });
      }
    },
    [applyTargetNodeSelection, targetNodeId, token]
  );

  const onCreateAgent = useCallback(async () => {
    if (!token || createAgentBusy) return;
    setCreateAgentBusy(true);
    setError(null);
    setWorktreeError(null);
    try {
      const normalizedTargetNodeId =
        targetNodeId.trim() && targetNodeId.trim() !== "main"
          ? targetNodeId.trim()
          : null;
      const name = agentName.trim() || "agent";
      const workdir = normalizeWorkdirInput(agentWorkdir);
      const normalizedRoot = normalizeWorkdirInput(
        resolveDefaultWorktreeRootForTargetNode(
          targetNodeId,
          agentNodes,
          defaultWorktreeRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
      const workdirPayload =
        worktreeMode === "create_worktree" &&
        normalizedRoot &&
        workdir === normalizedRoot
          ? ""
          : workdir;
      const preset = getAgentPreset(agentPresetId);
      const command = preset.command.trim();
      const args = preset.args.slice();
      if (!workdirPayload && worktreeMode !== "create_worktree") {
        setError("workdir is required");
        return;
      }
      if (worktreeMode !== "use_existing" && !worktreeRepo.trim()) {
        setError("worktree repo is required");
        return;
      }

      const agent = await api.createAgent(token, {
        name,
        command,
        args,
        workdir: workdirPayload,
        worktree_mode: worktreeMode,
        worktree_repo: worktreeRepo.trim() || null,
        worktree_ref: worktreeRef.trim() || null,
        code_mode: codeMode,
        target_node_id: normalizedTargetNodeId,
      });
      setAgents((prev) => [agent, ...prev]);
      setShowCreateAgent(false);
      setAgentName("");
      setAgentWorkdir(
        resolveWorkdirForModeChange(
          "",
          worktreeMode,
          selectedTargetNodeDefaultWorktreeRootRef.current,
          DEFAULT_WORKTREE_ROOT
        )
      );
      return agent;
    } catch (err: unknown) {
      const msg = formatWorktreeError(err) ?? stringifyApiError(err);
      if (msg.includes("worktree")) {
        setWorktreeError(msg);
      } else {
        setError(msg);
      }
      return null;
    } finally {
      setCreateAgentBusy(false);
    }
  }, [
    token,
    createAgentBusy,
    targetNodeId,
    agentName,
    agentWorkdir,
    agentNodes,
    defaultWorktreeRoot,
    worktreeMode,
    agentPresetId,
    worktreeRepo,
    worktreeRef,
    codeMode,
  ]);

  const onStartAgent = useCallback(async (id: string) => {
    if (!token) return null;
    setError(null);
    setWorktreeError(null);
    setStartingAgentIds((prev) => ({ ...prev, [id]: true }));
    try {
      const res = await api.startAgent(token, id);
      await refreshAgents();
      return res;
    } catch (err: unknown) {
      const message = formatWorktreeError(err) ?? stringifyApiError(err);
      if (message.toLowerCase().includes("agent already running")) {
        await refreshAgents();
        return null;
      }
      setError(message);
      return null;
    } finally {
      setStartingAgentIds((prev) => {
        if (!prev[id]) return prev;
        const next = { ...prev };
        delete next[id];
        return next;
      });
    }
  }, [token, refreshAgents]);

  const onStopAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    setWorktreeError(null);
    try {
      await api.stopAgent(token, id);
      await refreshAgents();
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token, refreshAgents]);

  const onDeleteAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    try {
      await api.deleteAgent(token, id);
      setAgents((prev) => prev.filter((agent) => agent.id !== id));
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token]);

  const onSetCodeMode = useCallback(async (id: string, next: boolean) => {
    if (!token) return;
    setError(null);
    try {
      await api.setAgentCodeMode(token, id, next);
      setAgents((prev) =>
        prev.map((agent) =>
          agent.id === id ? { ...agent, code_mode: next } : agent
        )
      );
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    }
  }, [token]);

  return {
    agents,
    setAgents,
    agentNodes,
    teams,
    agentNodeJoinBootstrap,
    agentNodeJoinBootstrapLoading,
    agentNodeJoinBootstrapError,
    error,
    setError,
    worktreeError,
    setWorktreeError,
    showCreateAgent,
    setShowCreateAgent,
    createAgentBusy,
    startingAgentIds,
    setStartingAgentIds,
    agentName,
    setAgentName,
    agentWorkdir,
    setAgentWorkdir,
    agentPresetId,
    setAgentPresetId,
    worktreeMode,
    setWorktreeMode: handleWorktreeModeChange,
    worktreeRepo,
    setWorktreeRepo,
    worktreeRef,
    setWorktreeRef,
    codeMode,
    setCodeMode,
    targetNodeId,
    applyTargetNodeSelection,
    nodeIdInput,
    setNodeIdInput,
    nodeNameInput,
    setNodeNameInput,
    nodeGrpcTargetInput,
    setNodeGrpcTargetInput,
    nodeTlsServerNameInput,
    setNodeTlsServerNameInput,
    nodeDefaultWorktreeRootInput,
    setNodeDefaultWorktreeRootInput,
    createAgentNodeBusy,
    updatingAgentNodeIds,
    deletingAgentNodeIds,
    teamMemberAgentsById,
    onCreateAgent,
    onStartAgent,
    onStopAgent,
    onDeleteAgent,
    onSetCodeMode,
    onCreateAgentNode,
    onUpdateAgentNode,
    onDeleteAgentNode,
    openCreateAgentModal,
    refreshAgents,
    refreshAgentNodes,
    refreshTeams,
    refreshAgentNodeJoinBootstrap,
    defaultWorktreeRoot,
  };
}
