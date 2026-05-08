import React, { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  parseApiErrorMessage,
  stringifyApiError,
} from "./api";
import {
  AGENT_NOT_RUNNING_ERROR,
} from "./agent_ws";
import {
  clearAuthAndRedirect,
} from "./auth_redirect";
import {
  canManageAgentNodes,
  clampAgentsPanelWidth,
  resolveAgentsPanelMaxWidth,
  resolveDefaultActiveAgentId,
} from "./app_agents_helpers";
import {
  buildWorkspaceNodePath,
  resolveAppRouteKind,
  resolvePostAuthRedirectTarget,
  resolveTeamRoute,
  isWorkspaceWorkbenchRoute,
  resolveWorkspaceAgentRoute,
  resolveWorkspaceLens,
  resolveWorkspaceNodeId,
  resolveWorkspaceNodeRoute,
  buildWorkspacePath,
  type WorkspaceLens,
} from "./app_route_selection";
import {
  OFFLINE_MESSAGE,
  sanitizeErrorBannerMessage,
  shouldHideErrorBannerMessage,
} from "./connection_status";
import {
  isNearBottom,
} from "./scroll";
import {
  buildAgentNodeSectionProps,
  buildCreateAgentModalProps,
  buildPermissionModalProps,
} from "./components/agents_route_modal_props";
import {
  buildAgentsWorkbenchProps,
  buildAgentsPanelProps,
  buildOutputHeaderProps,
} from "./components/agents_route_shell_props";
import { buildStandardWorkspaceLensItems } from "./components/workspace_lens_items";
import {
  getLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "./storage/safe_storage";
import {
  INPUT_HISTORY_STORAGE_KEY,
  parseInputHistory,
  pushInputHistory,
} from "./input_history";
import {
  AUTH_CARD_BASE_CLASS,
  AUTH_PAGE_CLASS,
  APP_ROOT_CLASS,
} from "./ui/tailwind_classes";
import { parseSendInputSessionMismatch } from "./app_utils";
import { resolveDefaultWorktreeRootForTargetNode } from "./worktree_defaults";
import { AdminRouteContainer } from "./routes/admin_route_container";
import { AgentsRouteContainer } from "./routes/agents_route_container";
import { RouteFallback } from "./routes/route_fallback";

import { useAppAuth } from "./use_app_auth";
import { useAppAgents } from "./use_app_agents";
import { useAppPermissions } from "./use_app_permissions";
import type { AcpPermissionLiveSignal } from "./use_app_permissions";
import { useAppOutputCache } from "./use_app_output_cache";
import { useAppAcpUi } from "./use_app_acp_ui";
import { useAppLayout } from "./use_app_layout";
import { useAppSseEvents } from "./use_app_sse_events";
import { useAppAdmin } from "./use_app_admin";
import { useAppPermissionState } from "./use_app_permission_state";

const AGENTS_DESKTOP_BREAKPOINT_PX = 1024;
const AGENTS_PANEL_COMPACT_ROWS_THRESHOLD = 320;

export {
  AGENT_SOURCE_MANUAL,
  AGENT_SOURCE_TEAM_FORGE,
  AGENT_EVENT_PAGE_SIZE,
} from "./api";
export {
  buildPermissionPollAgentIds,
  canManageAgentNodes,
  clampAgentsPanelWidth,
  decidePermissionJump,
  isSameAgentNodeRecordList,
  isSameAgentRecordList,
  loadAgentsPanelWidthPreference,
  persistAgentsPanelWidthPreference,
  removeAgentNodeRecord,
  replaceAgentNodeRecord,
  resolveActiveAcpView,
  resolveAgentsPanelMaxWidth,
  resolveDefaultActiveAgentId,
  resolveOutputHistoryKey,
  resolveSessionScopedEvents,
  upsertAgentNodeRecord,
} from "./app_agents_helpers";
export {
  buildGlobalPermissionPollAgentIds,
  buildPendingPermissionCountMap,
  chunkPermissionPollAgentIds,
  filterPermissionsForAgent,
  filterVisiblePermissionRecords,
  filterVisiblePermissionsForAgent,
  mergePendingPermissionCountMap,
  parsePermissionPollAgentIds,
  resolveGlobalPermissionPollIntervalMs,
  schedulePermissionPollLoop,
} from "./app_permission_polling";
export {
  resolveRuntimeKeyboardInset,
  resolveRuntimeViewportAxis,
  resolveRuntimeViewportSize,
  setupLayoutAnchorVarSync,
  setupRuntimeViewportVarSync,
  shouldSyncRuntimeViewportSize,
  toNonNegativeRoundedPx,
} from "./app_viewport";
export {
  isAgentsWorkbenchRoute,
  isTeamsRoute,
  isWorkspaceWorkbenchRoute,
  resolveAppRouteKind,
  resolveWorkspaceAgentRoute,
  resolvePostAuthRedirectTarget,
  resolveTeamRoute,
  shouldRedirectTeamsToLogin,
} from "./app_route_selection";

const LazyJoinPage = React.lazy(async () => {
  const module = (await import("./pages/join_page")) as typeof import("./pages/join_page");
  return { default: module.JoinPage };
});

const LazyTeamPage = React.lazy(async () => {
  const module = (await import("./pages/team_page")) as typeof import("./pages/team_page");
  return { default: module.TeamPage };
});

function AuthRedirect(): null {
  useEffect(() => {
    clearAuthAndRedirect(`${location.pathname}${location.search}${location.hash}`);
  }, []);
  return null;
}

function PostLoginRedirect({ target }: { target: string }): null {
  useEffect(() => {
    location.replace(target);
  }, [target]);
  return null;
}

export { RouteFallback } from "./routes/route_fallback";

function AuthGateCard({ title, message }: { title: string; message: string }) {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_BASE_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-notion-text">{title}</h2>
        <p className="mt-2 text-sm text-notion-text-muted">{message}</p>
      </section>
    </div>
  );
}

function AuthRequiredGate() {
  return <AuthGateCard title="Login Required" message="Please login to continue." />;
}

function ForbiddenRoute() {
  return (
    <AuthGateCard
      title="Forbidden"
      message="You do not have access to this page."
    />
  );
}

export function App() {
  const [routeLocation, setRouteLocation] = useState(() => ({
    pathname: location.pathname,
    search: location.search,
  }));
  const isAgentsRoute = isWorkspaceWorkbenchRoute(routeLocation.pathname);
  const isAdminRoute = routeLocation.pathname.startsWith("/admin");

  const {
    auth,
    authBusy,
    username,
    setUsername,
    displayName,
    setDisplayName,
    password,
    setPassword,
    error: authError,
    setAuth,
    onRegister,
    onLogin,
    onLogout,
  } = useAppAuth();

  const [agentsCollapsed, setAgentsCollapsed] = useState(true);

  const {
    agents,
    setAgents,
    agentNodes,
    teams,
    error: agentsError,
    worktreeError,
    showCreateAgent,
    setShowCreateAgent,
    createAgentBusy,
    startingAgentIds,
    agentName,
    setAgentName,
    agentWorkdir,
    setAgentWorkdir,
    agentPresetId,
    setAgentPresetId,
    worktreeMode,
    setWorktreeMode,
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
    agentNodeJoinBootstrap,
    agentNodeJoinBootstrapLoading,
    agentNodeJoinBootstrapError,
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
    defaultWorktreeRoot,
  } = useAppAgents(auth, isAgentsRoute);

  const [activeAgent, setActiveAgent] = useState<string | null>(null);

  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );

  const permissionState = useAppPermissionState();
  const [permissionLiveSignal, setPermissionLiveSignal] = useState<AcpPermissionLiveSignal>({
    seq: 0,
    agentIds: [],
  });
  const handleAcpPermissionSignal = useCallback((agentIds: string[]) => {
    if (agentIds.length === 0) return;
    setPermissionLiveSignal((prev) => ({
      seq: prev.seq + 1,
      agentIds,
    }));
  }, []);

  const {
    outputs,
    acpOutputs,
    activeSessionId,
    setActiveSessionId,
    agentSessions,
    setAgentSessions,
    eventMeta,
    loadAgentEvents,
    loadOlderEvents,
    consumeLiveOutputBatch,
  } = useAppOutputCache(
    auth,
    activeAgent,
    activeAgentRecord?.status ?? null,
    setAgents,
    handleAcpPermissionSignal
  );

  const {
    networkOnline,
    sseState,
    error: sseError,
    connectionBadge,
  } = useAppSseEvents(
    auth,
    isAgentsRoute,
    agents,
    activeAgent,
    activeAgentRecord?.status ?? null,
    consumeLiveOutputBatch,
    loadAgentEvents,
    refreshAgents
  );


  const [error, setError] = useState<string | null>(null);

  const {
    acpTab,
    setAcpTab,
    developerMode,
    setDeveloperMode,
    acpModeId,
    setAcpModeId,
    acpModelId,
    setAcpModelId,
    acpConfigId,
    setAcpConfigId,
    acpConfigValue,
    setAcpConfigValue,
    ansi,
    acpView,
    activeAgentModelLabel,
    isAgentActive,
    canControlAcp,
    canInterruptAcpRun,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onAcpCancel,
    onAcpClearSession,
  } = useAppAcpUi(auth?.token ?? null, activeAgent, activeAgentRecord, acpOutputs, permissionState.pendingPermissionCounts, setError);

  const {
    pendingPermissionCounts,
    scopedAcpPermissions,
    scopedAcpPermissionHistory,
  } = useAppPermissions(
    auth,
    isAgentsRoute,
    agents,
    activeAgent,
    agentsCollapsed,
    developerMode,
    acpTab,
    sseState === "connected",
    permissionLiveSignal,
    permissionState
  );


  const {
    safePaths,
    devices,
    audits,
    vapidInfo,
    passkeyEnabled,
    rootInitialized,
    selectedSafePaths,
    safePathInput,
    setSafePathInput,
    joinUrl,
    joinPin,
    joinToken,
    error: adminError,
    onPasskeyEnabledChange,
    onAddSafePath,
    onDeleteSafePath,
    onRevokeDevice,
    onRotateVapid,
    onCreateJoin,
    onToggleSafePath,
    onToggleAllSafePaths,
    onDeleteSelectedSafePaths,
  } = useAppAdmin(auth, isAdminRoute);

  const normalizedError = useMemo(() => {
    const rawError = error || authError || agentsError || sseError || adminError;
    if (!rawError) return null;
    const message = sanitizeErrorBannerMessage(rawError, networkOnline);
    return shouldHideErrorBannerMessage(message) ? null : message;
  }, [error, authError, agentsError, sseError, adminError, networkOnline]);

  const {
    agentsPanelWidth,
    setAgentsPanelWidth,
    appRootRef,
    appHeaderRef,
    workspaceRef,
  } = useAppLayout(auth, normalizedError, agentsCollapsed);

  const navigateWorkbenchRoute = useCallback(
    (pathname: string) => {
      const currentPath = `${routeLocation.pathname}${routeLocation.search}`;
      if (currentPath === pathname) {
        return;
      }
      window.history.pushState({}, "", pathname);
      window.dispatchEvent(new PopStateEvent("popstate"));
    },
    [routeLocation.pathname, routeLocation.search]
  );

  const [input, setInput] = useState("");
  const [inputHistory, setInputHistory] = useState<string[]>(() =>
    parseInputHistory(getLocalStorageItemSafe(INPUT_HISTORY_STORAGE_KEY))
  );
  const [inputHistoryCursor, setInputHistoryCursor] = useState(-1);
  const inputHistoryDraftRef = useRef("");
  const [terminalShowJump, setTerminalShowJump] = useState(false);
  const terminalRef = useRef<HTMLDivElement | null>(null);
  const terminalStickToBottomRef = useRef(true);
  const isComposingRef = useRef(false);
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const workspaceAgentRoute = useMemo(
    () => resolveWorkspaceAgentRoute(routeLocation.pathname),
    [routeLocation.pathname]
  );
  const workspaceNodeRoute = useMemo(
    () => resolveWorkspaceNodeRoute(routeLocation.pathname, routeLocation.search),
    [routeLocation.pathname, routeLocation.search]
  );
  const routeAgentId = workspaceAgentRoute?.mode === "agent" ? workspaceAgentRoute.agentId : null;
  const activeWorkspaceLens = useMemo(
    () =>
      workspaceNodeRoute
        ? "nodes"
        : (resolveWorkspaceLens(routeLocation.search) ?? "channels"),
    [routeLocation.search, workspaceNodeRoute]
  );
  const selectedWorkspaceNodeId = useMemo(
    () =>
      workspaceNodeRoute?.nodeId ??
      resolveWorkspaceNodeId(routeLocation.search),
    [routeLocation.search, workspaceNodeRoute]
  );
  const effectiveSelectedWorkspaceNodeId = selectedWorkspaceNodeId ?? "main";

  const workspaceLensItems = useMemo(
    () =>
      buildStandardWorkspaceLensItems(activeWorkspaceLens, {
        includeNodes: canManageAgentNodes(auth),
      }),
    [activeWorkspaceLens, auth]
  );

  const onSelectWorkspaceLens = useCallback(
    (value: string) => {
      const lens = value as WorkspaceLens;
      if (lens === "nodes") {
        navigateWorkbenchRoute(buildWorkspaceNodePath(selectedWorkspaceNodeId));
        return;
      }
      navigateWorkbenchRoute(buildWorkspacePath(activeAgent, lens));
    },
    [activeAgent, navigateWorkbenchRoute, selectedWorkspaceNodeId]
  );

  useEffect(() => {
    setLocalStorageItemSafe(
      INPUT_HISTORY_STORAGE_KEY,
      JSON.stringify(inputHistory)
    );
  }, [inputHistory]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onOnline = () => {
      setError(null);
    };
    const onOffline = () => {
      setError(OFFLINE_MESSAGE);
    };
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    return () => {
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
    };
  }, []);

  const handleTerminalScroll = useCallback(() => {
    const el = terminalRef.current;
    if (!el) return;
    const stick = isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight);
    if (stick === terminalStickToBottomRef.current) return;
    terminalStickToBottomRef.current = stick;
    setTerminalShowJump(!stick);
  }, []);

  const jumpToTerminalBottom = useCallback(() => {
    const el = terminalRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    terminalStickToBottomRef.current = true;
    setTerminalShowJump(false);
  }, []);

  const terminalOutputs = useMemo(
    () => outputs.filter((line) => line.stream !== "acp"),
    [outputs]
  );

  useEffect(() => {
    if (acpView.hasAcp) return;
    const el = terminalRef.current;
    if (!el) return;
    if (terminalStickToBottomRef.current) {
      el.scrollTop = el.scrollHeight;
      setTerminalShowJump(false);
      return;
    }
  }, [outputs, acpView.hasAcp]);

  useEffect(() => {
    terminalStickToBottomRef.current = true;
    setTerminalShowJump(false);
  }, [activeAgent, activeSessionId]);

  const isOutputLoading =
    Boolean(activeAgent) && eventMeta[`${activeAgent}:${activeSessionId ?? "latest"}`]?.loaded !== true;
  const isConversationLoading =
    Boolean(activeAgent) &&
    acpTab === "conversation" &&
    (activeAgentRecord?.code_mode ?? true) &&
    !acpView.hasAcp;

  const handleSelectAgent = useCallback((id: string) => {
    setActiveAgent(id);
    setActiveSessionId(agentSessions[id] ?? null);
    setAgentsCollapsed(true);
    navigateWorkbenchRoute(buildWorkspacePath(id, activeWorkspaceLens === "nodes" ? "channels" : activeWorkspaceLens));
  }, [agentSessions, navigateWorkbenchRoute, setActiveSessionId, activeWorkspaceLens]);

  const onRespondPermission = useCallback(async (
    agentId: string,
    permissionId: string,
    optionId?: string
  ) => {
    if (!auth?.token) return;
    setPermissionBusy(permissionId);
    try {
      await api.respondAcpPermission(auth.token, agentId, permissionId, {
        option_id: optionId ?? null,
        outcome: optionId ? undefined : "cancelled",
      });
    } catch (err: unknown) {
      setError(stringifyApiError(err));
    } finally {
      setPermissionBusy(null);
    }
  }, [auth?.token]);

  const onInputChange = useCallback(
    (value: string) => {
      setInput(value);
      if (inputHistoryCursor >= 0) {
        setInputHistoryCursor(-1);
      }
      inputHistoryDraftRef.current = value;
    },
    [inputHistoryCursor]
  );

  const onNavigateInputHistory = useCallback(
    (direction: "up" | "down") => {
      if (inputHistory.length === 0) return;
      if (direction === "up") {
        if (inputHistoryCursor < 0) {
          inputHistoryDraftRef.current = input;
          setInputHistoryCursor(0);
          setInput(inputHistory[0]);
          return;
        }
        const nextCursor = Math.min(inputHistory.length - 1, inputHistoryCursor + 1);
        setInputHistoryCursor(nextCursor);
        setInput(inputHistory[nextCursor]);
        return;
      }
      if (inputHistoryCursor < 0) return;
      if (inputHistoryCursor === 0) {
        setInputHistoryCursor(-1);
        setInput(inputHistoryDraftRef.current);
        return;
      }
      const nextCursor = inputHistoryCursor - 1;
      setInputHistoryCursor(nextCursor);
      setInput(inputHistory[nextCursor]);
    },
    [input, inputHistory, inputHistoryCursor]
  );

  const onSelectInputHistory = useCallback(
    (value: string) => {
      const nextCursor = inputHistory.findIndex((item) => item === value);
      setInputHistoryCursor(nextCursor);
      setInput(value);
      inputHistoryDraftRef.current = value;
    },
    [inputHistory]
  );

  const sendAcpInput = useCallback(async (
    rawText: string,
    options?: {
      recordHistory?: boolean;
      clearComposer?: boolean;
    }
  ) => {
    const text = rawText.trim();
    if (!text) return;
    if (!auth?.token || !activeAgent) return;
    
    let messageId: string | null = null;
    if (activeSessionId) {
      messageId =
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `local-${Date.now()}`;
    }
    const sendInputForSession = (sessionId: string | null) =>
      api.sendInput(
        auth.token,
        activeAgent,
        text,
        messageId ?? undefined,
        sessionId ?? undefined
      );
    try {
      await sendInputForSession(activeSessionId);
      if (options?.recordHistory) {
        setInputHistory((prev) => pushInputHistory(prev, text));
        setInputHistoryCursor(-1);
      }
      if (options?.clearComposer) {
        inputHistoryDraftRef.current = "";
        setInput("");
      }
      } catch (err: unknown) {
        const msg = parseApiErrorMessage(err) ?? String(err || "websocket not connected");
        const sessionMismatch = parseSendInputSessionMismatch(msg);
        if (sessionMismatch) {
          const runningSessionId = sessionMismatch.running;
          setActiveSessionId(runningSessionId);
          setAgentSessions((prev) => ({ ...prev, [activeAgent]: runningSessionId }));
          await loadAgentEvents(activeAgent, runningSessionId);
          try {
            await sendInputForSession(runningSessionId);
            if (options?.recordHistory) {
              setInputHistory((prev) => pushInputHistory(prev, text));
              setInputHistoryCursor(-1);
            }
            if (options?.clearComposer) {
              inputHistoryDraftRef.current = "";
              setInput("");
            }
            return;
          } catch (retryErr: unknown) {
            const retryMsg = parseApiErrorMessage(retryErr) ?? String(retryErr || "websocket not connected");
            setError(retryMsg);
            if (retryMsg.includes(AGENT_NOT_RUNNING_ERROR)) {
              await refreshAgents();
            }
            return;
          }
        }
        setError(msg);
        if (msg.includes(AGENT_NOT_RUNNING_ERROR)) {
          await refreshAgents();
        }
      }
  }, [
    auth?.token,
    activeAgent,
    activeSessionId,
    loadAgentEvents,
    refreshAgents,
    setActiveSessionId,
    setAgentSessions
  ]);

  const handleAgentsSplitterPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (typeof window === "undefined") return;
      if (window.innerWidth <= AGENTS_DESKTOP_BREAKPOINT_PX) return;
      const workspace = workspaceRef.current;
      if (!workspace) return;

      event.preventDefault();

      const startX = event.clientX;
      const startWidth = agentsPanelWidth;
      const bodyStyle = document.body.style;
      const previousCursor = bodyStyle.cursor;
      const previousUserSelect = bodyStyle.userSelect;
      bodyStyle.cursor = "col-resize";
      bodyStyle.userSelect = "none";

      const onPointerMove = (moveEvent: PointerEvent) => {
        const workspaceWidth = workspace.getBoundingClientRect().width;
        const nextMaxWidth = resolveAgentsPanelMaxWidth(workspaceWidth);
        const nextWidth = clampAgentsPanelWidth(
          startWidth + (moveEvent.clientX - startX),
          nextMaxWidth
        );
        setAgentsPanelWidth(nextWidth);
      };

      const onPointerUp = () => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
        window.removeEventListener("pointercancel", onPointerUp);
        bodyStyle.cursor = previousCursor;
        bodyStyle.userSelect = previousUserSelect;
      };

      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
      window.addEventListener("pointercancel", onPointerUp);
    },
    [agentsPanelWidth, workspaceRef, setAgentsPanelWidth]
  );

  const handleCollapseAgents = useCallback(() => {
    setAgentsCollapsed(true);
  }, []);

  const handleExpandAgents = useCallback(() => {
    setAgentsCollapsed(false);
  }, []);

  useEffect(() => {
    if (agents.length === 0) {
      if (activeAgent !== null) {
        setActiveAgent(null);
      }
      if (activeSessionId !== null) {
        setActiveSessionId(null);
      }
      return;
    }

    if (routeAgentId) {
      const routeAgentStillExists = agents.some((agent) => agent.id === routeAgentId);
      if (routeAgentStillExists) {
        if (routeAgentId !== activeAgent) {
          setActiveAgent(routeAgentId);
        }
        const nextSessionId = agentSessions[routeAgentId] ?? null;
        if (nextSessionId !== activeSessionId) {
          setActiveSessionId(nextSessionId);
        }
        return;
      }
    }

    if (activeAgent) {
      const activeAgentStillExists = agents.some((agent) => agent.id === activeAgent);
      if (activeAgentStillExists) {
        return;
      }
    }

    const nextAgentId = resolveDefaultActiveAgentId(agents);
    if (nextAgentId) {
      if (nextAgentId !== activeAgent) {
        setActiveAgent(nextAgentId);
      }
      const nextSessionId = agentSessions[nextAgentId] ?? null;
      if (nextSessionId !== activeSessionId) {
        setActiveSessionId(nextSessionId);
      }
    } else {
      if (activeAgent !== null) {
        setActiveAgent(null);
      }
      if (activeSessionId !== null) {
        setActiveSessionId(null);
      }
    }
  }, [
    agents,
    activeAgent,
    activeSessionId,
    agentSessions,
    routeAgentId,
    setActiveAgent,
    setActiveSessionId,
  ]);

  useEffect(() => {
    if (auth?.token) {
      return;
    }
    if (activeAgent !== null) {
      setActiveAgent(null);
    }
    if (activeSessionId !== null) {
      setActiveSessionId(null);
    }
  }, [activeAgent, activeSessionId, auth?.token, setActiveAgent, setActiveSessionId]);

  const closeCreateAgentModal = useCallback(() => {
    setShowCreateAgent(false);
  }, [setShowCreateAgent]);

  const postAuthRedirectTarget = resolvePostAuthRedirectTarget(
    routeLocation.pathname,
    routeLocation.search,
    auth,
    auth?.token ?? null
  );
  const routeKind = resolveAppRouteKind(
    routeLocation,
    auth,
    auth?.token ?? null,
    postAuthRedirectTarget
  );

  useEffect(() => {
    const syncRouteLocation = () => {
      setRouteLocation({
        pathname: location.pathname,
        search: location.search,
      });
    };
    window.addEventListener("popstate", syncRouteLocation);
    return () => {
      window.removeEventListener("popstate", syncRouteLocation);
    };
  }, []);

  const workspaceStyle = useMemo<React.CSSProperties | undefined>(() => {
    if (agentsCollapsed) {
      return undefined;
    }
    return {
      ["--agents-panel-width" as string]: `${agentsPanelWidth}px`,
    };
  }, [agentsCollapsed, agentsPanelWidth]);

  const agentsPanelShowsCompactRows =
    !agentsCollapsed && agentsPanelWidth <= AGENTS_PANEL_COMPACT_ROWS_THRESHOLD;

  const hasPendingPermissions = useMemo(() => {
    return Object.values(pendingPermissionCounts).some((count) => count > 0);
  }, [pendingPermissionCounts]);

  const agentsPanelProps = useMemo(
    () =>
      buildAgentsPanelProps({
        agents,
        activeAgent,
        agentsCollapsed,
        compactRows: agentsPanelShowsCompactRows,
        hasPendingPermissions,
        pendingPermissionCounts,
        startingAgentIds,
        onCollapse: handleCollapseAgents,
        onExpand: handleExpandAgents,
        onCreateAgent: openCreateAgentModal,
        onSelectAgent: handleSelectAgent,
        onToggleCodeMode: onSetCodeMode,
        onStartAgent: onStartAgent,
        onStopAgent: onStopAgent,
        onDeleteAgent: onDeleteAgent,
      }),
    [
      agents,
      activeAgent,
      agentsCollapsed,
      agentsPanelShowsCompactRows,
      hasPendingPermissions,
      pendingPermissionCounts,
      startingAgentIds,
      handleCollapseAgents,
      handleExpandAgents,
      openCreateAgentModal,
      handleSelectAgent,
      onSetCodeMode,
      onStartAgent,
      onStopAgent,
      onDeleteAgent,
    ]
  );

  const outputHeaderProps = useMemo(
    () =>
      buildOutputHeaderProps({
        activeAgent: activeAgentRecord,
        activeSessionId,
        developerMode,
        hasAcp: acpView.hasAcp,
        thinkingStartTs: acpView.thinkingStartTs,
        runStatus: acpView.runStatus?.status ?? null,
        modelLabel: activeAgentModelLabel,
      }),
    [
      activeAgentRecord,
      activeSessionId,
      developerMode,
      acpView.hasAcp,
      acpView.thinkingStartTs,
      acpView.runStatus?.status,
      activeAgentModelLabel,
    ]
  );
  const workbenchProps = useMemo(
    () =>
      buildAgentsWorkbenchProps({
        activeAgent,
        activeAgentRecord,
        activeSessionId,
        developerMode,
        acpTab,
        acpView,
        eventMeta,
        isAgentActive,
        outputs,
        terminalOutputs,
        scopedAcpPermissionHistory,
        isOutputLoading,
        isConversationLoading,
        terminalRef,
        input,
        inputHistory,
        ansi,
        canControlAcp,
        canInterruptAcpRun,
        acpModeId,
        acpModelId,
        acpConfigId,
        acpConfigValue,
        isComposingRef,
        onLoadOlderEvents: loadOlderEvents,
        onTerminalScroll: handleTerminalScroll,
        onSelectTab: setAcpTab,
        onAcpModeIdChange: setAcpModeId,
        onAcpModelIdChange: setAcpModelId,
        onAcpConfigIdChange: setAcpConfigId,
        onAcpConfigValueChange: setAcpConfigValue,
        onAcpSetMode: onAcpSetMode,
        onAcpSetModel: onAcpSetModel,
        onAcpSetConfig: onAcpSetConfig,
        onAcpCancel: onAcpCancel,
        onAcpClearSession: onAcpClearSession,
        onInputChange: onInputChange,
        onSelectInputHistory: onSelectInputHistory,
        onNavigateInputHistory: onNavigateInputHistory,
        onSendAcpInput: sendAcpInput,
        onJumpToTerminalBottom: jumpToTerminalBottom,
        showTerminalJump: terminalShowJump,
      }),
    [
      activeAgent,
      activeAgentRecord,
      activeSessionId,
      developerMode,
      acpTab,
      acpView,
      eventMeta,
      isAgentActive,
      outputs,
      terminalOutputs,
      scopedAcpPermissionHistory,
      isOutputLoading,
      isConversationLoading,
      terminalRef,
      input,
      inputHistory,
      ansi,
      canControlAcp,
      canInterruptAcpRun,
      acpModeId,
      acpModelId,
      acpConfigId,
      acpConfigValue,
      isComposingRef,
      loadOlderEvents,
      handleTerminalScroll,
      setAcpTab,
      setAcpModeId,
      setAcpModelId,
      setAcpConfigId,
      setAcpConfigValue,
      onAcpSetMode,
      onAcpSetModel,
      onAcpSetConfig,
      onAcpCancel,
      onAcpClearSession,
      onInputChange,
      onSelectInputHistory,
      onNavigateInputHistory,
      sendAcpInput,
      jumpToTerminalBottom,
      terminalShowJump,
    ]
  );
  const canManageNodes = canManageAgentNodes(auth);

  const handleSelectWorkspaceNode = useCallback(
    (nodeId: string) => navigateWorkbenchRoute(buildWorkspaceNodePath(nodeId)),
    [navigateWorkbenchRoute]
  );
  const handleOpenNodeAgent = useCallback(
    (agentId: string) => {
      setActiveAgent(agentId);
      setActiveSessionId(agentSessions[agentId] ?? null);
      setAgentsCollapsed(true);
      navigateWorkbenchRoute(buildWorkspacePath(agentId, "channels"));
    },
    [
      agentSessions,
      navigateWorkbenchRoute,
      setActiveAgent,
      setActiveSessionId,
      setAgentsCollapsed,
    ]
  );
  const createAgentDefaultWorktreeRoot = useMemo(
    () =>
      resolveDefaultWorktreeRootForTargetNode(
        targetNodeId,
        agentNodes,
        defaultWorktreeRoot
      ),
    [targetNodeId, agentNodes, defaultWorktreeRoot]
  );

  const createAgentModalProps = useMemo(
    () =>
      buildCreateAgentModalProps({
        agentName,
        setAgentName,
        agentWorkdir,
        setAgentWorkdir,
        agentPresetId,
        setAgentPresetId,
        worktreeMode,
        setWorktreeMode,
        worktreeRepo,
        setWorktreeRepo,
        worktreeRef,
        setWorktreeRef,
        codeMode,
        setCodeMode,
        worktreeError,
        createBusy: createAgentBusy,
        workdirPlaceholder: createAgentDefaultWorktreeRoot,
        onCreateAgent: onCreateAgent,
        onClose: closeCreateAgentModal,
      }),
    [
      agentName,
      setAgentName,
      agentWorkdir,
      setAgentWorkdir,
      agentPresetId,
      setAgentPresetId,
      worktreeMode,
      setWorktreeMode,
      worktreeRepo,
      setWorktreeRepo,
      worktreeRef,
      setWorktreeRef,
      codeMode,
      setCodeMode,
      worktreeError,
      createAgentBusy,
      createAgentDefaultWorktreeRoot,
      onCreateAgent,
      closeCreateAgentModal,
    ]
  );

  const agentNodeSectionProps = useMemo(
    () =>
      buildAgentNodeSectionProps(
        canManageNodes
          ? {
              nodes: agentNodes,
              agents,
              targetNodeId,
              onTargetNodeIdChange: applyTargetNodeSelection,
              nodeIdInput,
              onNodeIdInputChange: setNodeIdInput,
              nodeNameInput,
              onNodeNameInputChange: setNodeNameInput,
              grpcTargetInput: nodeGrpcTargetInput,
              onGrpcTargetInputChange: setNodeGrpcTargetInput,
              tlsServerNameInput: nodeTlsServerNameInput,
              onTlsServerNameInputChange: setNodeTlsServerNameInput,
              defaultWorktreeRootInput: nodeDefaultWorktreeRootInput,
              onDefaultWorktreeRootInputChange: setNodeDefaultWorktreeRootInput,
              createBusy: createAgentNodeBusy,
              updatingNodeIds: updatingAgentNodeIds,
              deletingNodeIds: deletingAgentNodeIds,
              nodeJoinBootstrap: agentNodeJoinBootstrap,
              nodeJoinBootstrapLoading: agentNodeJoinBootstrapLoading,
              nodeJoinBootstrapError: agentNodeJoinBootstrapError,
              onCreateNode: onCreateAgentNode,
              onUpdateNode: onUpdateAgentNode,
              onDeleteNode: onDeleteAgentNode,
              onOpenNodeDetail: (nodeId: string) =>
                navigateWorkbenchRoute(buildWorkspaceNodePath(nodeId)),
            }
          : null
      ),
    [
      canManageNodes,
      agentNodes,
      agents,
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
      agentNodeJoinBootstrap,
      agentNodeJoinBootstrapLoading,
      agentNodeJoinBootstrapError,
      onCreateAgentNode,
      onUpdateAgentNode,
      onDeleteAgentNode,
      navigateWorkbenchRoute,
    ]
  );

  const permissionModalProps = useMemo(
    () =>
      buildPermissionModalProps(
        activeAgent && scopedAcpPermissions.length > 0
          ? {
              permissions: scopedAcpPermissions,
              permissionBusy,
              onRespond: onRespondPermission,
            }
          : null
      ),
    [activeAgent, scopedAcpPermissions, permissionBusy, onRespondPermission]
  );

  switch (routeKind) {
    case "join":
      return (
        <div className={APP_ROOT_CLASS} ref={appRootRef}>
          <Suspense fallback={<RouteFallback label="Loading join flow..." />}>
            <LazyJoinPage onComplete={(next) => setAuth(next)} />
          </Suspense>
        </div>
      );
    case "admin-auth-required":
      return <AuthRequiredGate />;
    case "admin-forbidden":
      return <ForbiddenRoute />;
    case "admin":
      return (
        <AdminRouteContainer
          appRootRef={appRootRef}
          auth={auth!}
          error={normalizedError}
          setError={setError}
          safePaths={safePaths}
          selectedSafePaths={selectedSafePaths}
          onToggleSafePath={onToggleSafePath}
          onToggleAllSafePaths={onToggleAllSafePaths}
          onDeleteSelectedSafePaths={onDeleteSelectedSafePaths}
          devices={devices}
          audits={audits}
          vapidInfo={vapidInfo}
          onRotateVapid={onRotateVapid}
          onAddSafePath={onAddSafePath}
          onDeleteSafePath={onDeleteSafePath}
          onRevokeDevice={onRevokeDevice}
          onCreateJoin={onCreateJoin}
          joinUrl={joinUrl}
          joinToken={joinToken}
          joinPin={joinPin}
          safePathInput={safePathInput}
          setSafePathInput={setSafePathInput}
          developerMode={developerMode}
          onDeveloperModeChange={setDeveloperMode}
          passkeyEnabled={passkeyEnabled}
          onPasskeyEnabledChange={onPasskeyEnabledChange}
        />
      );
    case "teams-auth-redirect":
      return <AuthRedirect />;
    case "teams": {
      const teamRoute = resolveTeamRoute(routeLocation.pathname);
      return (
        <div className={APP_ROOT_CLASS} ref={appRootRef}>
          <Suspense fallback={<RouteFallback label="Loading teams..." />}>
            <LazyTeamPage
              auth={auth!}
              token={auth!.token}
              onLogout={onLogout}
              developerMode={developerMode}
              routeTeamId={teamRoute?.teamId ?? null}
              routeSearch={routeLocation.search}
              defaultWorktreeRoot={defaultWorktreeRoot}
            />
          </Suspense>
        </div>
      );
    }
    case "post-auth-redirect":
      return <PostLoginRedirect target={postAuthRedirectTarget!} />;
    case "workspace":
      return (
        <AgentsRouteContainer
          activeWorkspaceLens={activeWorkspaceLens}
          appRootRef={appRootRef}
          appHeaderRef={appHeaderRef}
          auth={auth}
          normalizedError={normalizedError}
          onClearError={() => setError(null)}
          authBusy={authBusy}
          rootInitialized={rootInitialized}
          username={username}
          password={password}
          displayName={displayName}
          setUsername={setUsername}
          setPassword={setPassword}
          setDisplayName={setDisplayName}
          onLogin={onLogin}
          onRegister={onRegister}
          agentsCollapsed={agentsCollapsed}
          onCollapseAgents={handleCollapseAgents}
          onExpandAgents={handleExpandAgents}
          connectionBadge={connectionBadge}
          onLogout={onLogout}
          navigateWorkbenchRoute={navigateWorkbenchRoute}
          workspaceRef={workspaceRef}
          workspaceStyle={workspaceStyle}
          onAgentsSplitterPointerDown={handleAgentsSplitterPointerDown}
          agentsPanelProps={agentsPanelProps}
          outputHeaderProps={outputHeaderProps}
          workbenchProps={workbenchProps}
          showCreateAgent={showCreateAgent}
          createAgentModalProps={createAgentModalProps}
          agentNodeSectionProps={agentNodeSectionProps}
          permissionModalProps={permissionModalProps}
          lensItems={workspaceLensItems}
          onSelectLens={onSelectWorkspaceLens}
          nodes={agentNodes}
          agents={agents}
          teams={teams}
          teamMemberAgentsById={teamMemberAgentsById}
          selectedNodeId={effectiveSelectedWorkspaceNodeId}
          nodeJoinBootstrap={agentNodeJoinBootstrap}
          nodeJoinBootstrapLoading={agentNodeJoinBootstrapLoading}
          nodeJoinBootstrapError={agentNodeJoinBootstrapError}
          updatingNodeIds={updatingAgentNodeIds}
          deletingNodeIds={deletingAgentNodeIds}
          onSelectNode={handleSelectWorkspaceNode}
          onOpenNodeAgent={handleOpenNodeAgent}
          onCreateAgent={openCreateAgentModal}
          onUpdateNode={onUpdateAgentNode}
          onDeleteNode={onDeleteAgentNode}
        />
      );
  }
}
