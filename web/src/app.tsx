import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  AgentRecord,
  AuditRecord,
  AcpPermissionRecord,
  DeviceRecord,
  SafePath,
  VapidInfo,
} from "./api";
import { buildAcpView } from "./acp";
import {
  AGENT_NOT_RUNNING_ERROR,
  shouldIgnoreAgentWsError,
  sanitizeAgentError,
  isAgentActiveStatus,
} from "./agent_ws";
import { ErrorBanner } from "./error_banner";
import { clearAuthAndRedirect, isInvalidTokenMessage } from "./auth_redirect";
import {
  deriveConnectionBadge,
  OFFLINE_MESSAGE,
  sanitizeErrorBannerMessage,
  shouldHideErrorBannerMessage,
  type SseConnectionState,
  UPSTREAM_HTML_MESSAGE,
} from "./connection_status";
import {
  getAdaptivePollInterval,
  EventCursor,
  getMaxEventCursor,
  isCursorNewer,
  updateLastEventCursor,
} from "./event_polling";
import { compareEventOrder } from "./seq_order";
import {
  appendOutputLine,
  buildAcpCacheSlice,
  buildOutputCacheSlice,
  isSameOutputList,
  mergeOutputsPreserveHistory,
  mergeOutputs,
  OutputLine,
  selectCachedOutputs,
} from "./output_cache";
import { isNearBottom } from "./scroll";
import { escapeHtml } from "./markdown";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentModelLabel,
  getAgentPreset,
  type AgentPresetId,
} from "./agent_presets";
import { AgentsPanel } from "./components/agents_panel";
import { CreateAgentModal } from "./components/create_agent_modal";
import { InputDock } from "./components/input_dock";
import { OutputHeader } from "./components/output_header";
import { OutputBody } from "./components/output_body";
import { OutputErrorBoundary } from "./components/output_error_boundary";
import { PermissionModal } from "./components/permission_modal";
import { getAcpConversationCacheStats } from "./components/acp_conversation";
import { useAcpConversation } from "./hooks/use_acp_conversation";
import { loadOutputCaches, saveOutputCaches } from "./storage/output_cache_storage";
import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "./storage/safe_storage";
import { AdminPage } from "./pages/admin_page";
import { AuthRequired, ForbiddenPage } from "./pages/auth_pages";
import { JoinPage } from "./pages/join_page";
import { TeamPage } from "./pages/team_page";
import { ensurePushSubscription } from "./push";
import {
  INPUT_HISTORY_STORAGE_KEY,
  parseInputHistory,
  pushInputHistory,
} from "./input_history";
import {
  loginCredentialToJson,
  publicKeyCredentialCreationOptionsFromJson,
  publicKeyCredentialRequestOptionsFromJson,
  registerCredentialToJson,
} from "./webauthn";
import { AuthState } from "./types";
import {
  normalizeRuntimeWorktreeRoot,
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
  resolveWorkdirForModalOpen,
} from "./worktree_defaults";
import { buildSseTargetAgentIds, encodeSseTargetAgentIds } from "./sse_targets";

const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
const PERMISSION_JUMP_MAX_ATTEMPTS = 24;
const PERMISSION_JUMP_RETRY_DELAY_MS = 120;

type PendingPermissionJumpState = {
  toolCallId: string;
  sessionId: string | null;
  attempts: number;
};

type RuntimeViewportSize = {
  height: number;
  width: number;
};

type PermissionJumpDecision = "idle" | "wait" | "clear" | "attempt";

type RuntimeEventHandler = () => void;

type RuntimeEventTargetLike = {
  addEventListener: (type: string, listener: RuntimeEventHandler) => void;
  removeEventListener: (type: string, listener: RuntimeEventHandler) => void;
};

type RuntimeVisualViewportLike = RuntimeEventTargetLike & {
  height?: number;
  width?: number;
};

type RuntimeWindowLike = RuntimeEventTargetLike & {
  innerHeight: number;
  innerWidth: number;
  visualViewport?: RuntimeVisualViewportLike | null;
  requestAnimationFrame?: (cb: (timestamp: number) => void) => number;
  cancelAnimationFrame?: (id: number) => void;
};

type StyleVarTarget = {
  setProperty: (name: string, value: string) => void;
};

type LayoutAnchorNodeLike = {
  getBoundingClientRect: () => { height: number; top: number };
};

type LayoutAnchorNodes = {
  appRoot: LayoutAnchorNodeLike | null;
  appHeader: LayoutAnchorNodeLike | null;
  workspace: LayoutAnchorNodeLike | null;
};

type ResizeObserverLike = {
  observe: (target: object) => void;
  disconnect: () => void;
};

type ResizeObserverCtorLike = new (callback: () => void) => ResizeObserverLike;

export function resolveRuntimeViewportSize(
  viewport: Pick<VisualViewport, "height" | "width"> | null | undefined,
  innerHeight: number,
  innerWidth: number
): RuntimeViewportSize {
  const toSafeViewportDimension = (
    viewportValue: number | undefined,
    fallback: number
  ): number => {
    const safeFallback =
      typeof fallback === "number" && Number.isFinite(fallback) && fallback > 0
        ? fallback
        : 1;
    if (
      typeof viewportValue !== "number" ||
      !Number.isFinite(viewportValue) ||
      viewportValue <= 1
    ) {
      return safeFallback;
    }
    return viewportValue;
  };
  return {
    height: Math.max(1, Math.round(toSafeViewportDimension(viewport?.height, innerHeight))),
    width: Math.max(1, Math.round(toSafeViewportDimension(viewport?.width, innerWidth))),
  };
}

export function shouldSyncRuntimeViewportSize(
  previous: RuntimeViewportSize | null,
  next: RuntimeViewportSize
): boolean {
  if (!previous) return true;
  return previous.height !== next.height || previous.width !== next.width;
}

export function toNonNegativeRoundedPx(value: number | null | undefined): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  return Math.max(0, Math.round(value));
}

export function decidePermissionJump(
  pending: PendingPermissionJumpState | null,
  acpTab: "conversation" | "debug",
  activeSessionId: string | null,
  maxAttempts: number = PERMISSION_JUMP_MAX_ATTEMPTS
): PermissionJumpDecision {
  if (!pending) return "idle";
  if (acpTab !== "conversation") return "wait";
  if (pending.sessionId && activeSessionId !== pending.sessionId) return "wait";
  if (pending.attempts >= maxAttempts) return "clear";
  return "attempt";
}

export function setupRuntimeViewportVarSync(
  runtimeWindow: RuntimeWindowLike,
  styleTarget: StyleVarTarget
): () => void {
  const viewport = runtimeWindow.visualViewport;
  let rafId: number | null = null;
  let previousSize: RuntimeViewportSize | null = null;
  const syncViewportSizeNow = () => {
    const nextSize = resolveRuntimeViewportSize(
      viewport,
      runtimeWindow.innerHeight,
      runtimeWindow.innerWidth
    );
    if (!shouldSyncRuntimeViewportSize(previousSize, nextSize)) {
      return;
    }
    previousSize = nextSize;
    styleTarget.setProperty("--agenthub-vh", `${nextSize.height}px`);
    styleTarget.setProperty("--agenthub-vw", `${nextSize.width}px`);
  };
  const scheduleSyncViewportSize = () => {
    if (
      typeof runtimeWindow.requestAnimationFrame !== "function" ||
      typeof runtimeWindow.cancelAnimationFrame !== "function"
    ) {
      syncViewportSizeNow();
      return;
    }
    if (rafId != null) return;
    rafId = runtimeWindow.requestAnimationFrame(() => {
      rafId = null;
      syncViewportSizeNow();
    });
  };
  syncViewportSizeNow();
  runtimeWindow.addEventListener("resize", scheduleSyncViewportSize);
  runtimeWindow.addEventListener("orientationchange", scheduleSyncViewportSize);
  viewport?.addEventListener("resize", scheduleSyncViewportSize);
  viewport?.addEventListener("scroll", scheduleSyncViewportSize);
  return () => {
    if (
      rafId != null &&
      typeof runtimeWindow.cancelAnimationFrame === "function"
    ) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    runtimeWindow.removeEventListener("resize", scheduleSyncViewportSize);
    runtimeWindow.removeEventListener("orientationchange", scheduleSyncViewportSize);
    viewport?.removeEventListener("resize", scheduleSyncViewportSize);
    viewport?.removeEventListener("scroll", scheduleSyncViewportSize);
  };
}

export function setupLayoutAnchorVarSync(
  runtimeWindow: RuntimeWindowLike,
  styleTarget: StyleVarTarget,
  nodes: LayoutAnchorNodes,
  resizeObserverCtor?: ResizeObserverCtorLike
): () => void {
  const syncLayoutAnchors = () => {
    const headerHeight = toNonNegativeRoundedPx(
      nodes.appHeader?.getBoundingClientRect().height
    );
    if (headerHeight != null) {
      styleTarget.setProperty("--agenthub-header-height", `${headerHeight}px`);
    }
    const workspaceTop = toNonNegativeRoundedPx(
      nodes.workspace?.getBoundingClientRect().top
    );
    if (workspaceTop != null) {
      styleTarget.setProperty("--agenthub-workspace-top", `${workspaceTop}px`);
    }
  };
  let rafId: number | null = null;
  const scheduleSync = () => {
    if (
      typeof runtimeWindow.requestAnimationFrame !== "function" ||
      typeof runtimeWindow.cancelAnimationFrame !== "function"
    ) {
      syncLayoutAnchors();
      return;
    }
    if (rafId != null) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    rafId = runtimeWindow.requestAnimationFrame(() => {
      rafId = null;
      syncLayoutAnchors();
    });
  };

  syncLayoutAnchors();
  runtimeWindow.addEventListener("resize", scheduleSync);
  runtimeWindow.addEventListener("orientationchange", scheduleSync);
  const viewport = runtimeWindow.visualViewport;
  viewport?.addEventListener("resize", scheduleSync);
  viewport?.addEventListener("scroll", scheduleSync);
  let observer: ResizeObserverLike | null = null;
  if (resizeObserverCtor) {
    observer = new resizeObserverCtor(() => scheduleSync());
    if (nodes.appRoot) observer.observe(nodes.appRoot);
    if (nodes.appHeader) observer.observe(nodes.appHeader);
    if (nodes.workspace) observer.observe(nodes.workspace);
  }
  return () => {
    if (
      rafId != null &&
      typeof runtimeWindow.cancelAnimationFrame === "function"
    ) {
      runtimeWindow.cancelAnimationFrame(rafId);
    }
    runtimeWindow.removeEventListener("resize", scheduleSync);
    runtimeWindow.removeEventListener("orientationchange", scheduleSync);
    viewport?.removeEventListener("resize", scheduleSync);
    viewport?.removeEventListener("scroll", scheduleSync);
    observer?.disconnect();
  };
}

export function schedulePermissionPollLoop(
  delay: number,
  pollState: { timer: number | null },
  pollOnce: () => Promise<number>,
  isCancelled: () => boolean,
  scheduleTimeout: (callback: () => void, delayMs: number) => number = (callback, delayMs) =>
    window.setTimeout(callback, delayMs),
  clearTimeoutFn: (timerId: number) => void = (timerId) => window.clearTimeout(timerId)
): void {
  if (isCancelled()) return;
  if (pollState.timer != null) {
    clearTimeoutFn(pollState.timer);
    pollState.timer = null;
  }
  pollState.timer = scheduleTimeout(async () => {
    if (isCancelled()) {
      pollState.timer = null;
      return;
    }
    const pendingCount = await pollOnce();
    if (isCancelled()) {
      pollState.timer = null;
      return;
    }
    const nextDelay = pendingCount > 0 ? 5_000 : 3_000;
    schedulePermissionPollLoop(
      nextDelay,
      pollState,
      pollOnce,
      isCancelled,
      scheduleTimeout,
      clearTimeoutFn
    );
  }, delay);
}

export function App() {
  const eventLimit = 200;
  const maxCachedEvents = 800;
  const maxCachedSessions = 40;
  const [auth, setAuth] = useState<AuthState | null>(() => {
    const raw = getLocalStorageItemSafe("agenthub_auth");
    if (!raw) return null;
    try {
      return JSON.parse(raw) as AuthState;
    } catch {
      removeLocalStorageItemSafe("agenthub_auth");
      return null;
    }
  });
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [agents, setAgents] = useState<AgentRecord[]>([]);
  const [safePaths, setSafePaths] = useState<SafePath[]>([]);
  const [devices, setDevices] = useState<DeviceRecord[]>([]);
  const [audits, setAudits] = useState<AuditRecord[]>([]);
  const [selectedSafePaths, setSelectedSafePaths] = useState<Set<string>>(
    () => new Set()
  );
  const [agentName, setAgentName] = useState("");
  const [agentWorkdir, setAgentWorkdir] = useState("");
  const [defaultWorktreeRoot, setDefaultWorktreeRoot] = useState(
    DEFAULT_WORKTREE_ROOT
  );
  const [agentPresetId, setAgentPresetId] = useState<AgentPresetId>(
    DEFAULT_AGENT_PRESET_ID
  );
  const [worktreeMode, setWorktreeMode] = useState<
    "use_existing" | "create_worktree" | "reuse_worktree"
  >("use_existing");
  const [worktreeRepo, setWorktreeRepo] = useState("");
  const [worktreeRef, setWorktreeRef] = useState("");
  const [codeMode, setCodeMode] = useState(true);
  const [acpModeId, setAcpModeId] = useState("");
  const [acpModelId, setAcpModelId] = useState("");
  const [acpConfigId, setAcpConfigId] = useState("");
  const [acpConfigValue, setAcpConfigValue] = useState("");
  const [safePathInput, setSafePathInput] = useState("");
  const [joinQr, setJoinQr] = useState<string | null>(null);
  const [joinPin, setJoinPin] = useState<string | null>(null);
  const [joinToken, setJoinToken] = useState<string | null>(null);
  const [vapidInfo, setVapidInfo] = useState<VapidInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [networkOnline, setNetworkOnline] = useState<boolean>(getNavigatorOnline);
  const [sseState, setSseState] = useState<SseConnectionState>("idle");
  const [worktreeError, setWorktreeError] = useState<string | null>(null);
  const [activeAgent, setActiveAgent] = useState<string | null>(null);
  const [outputs, setOutputs] = useState<OutputLine[]>([]);
  const [acpOutputs, setAcpOutputs] = useState<OutputLine[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [agentSessions, setAgentSessions] = useState<Record<string, string>>(
    {}
  );
  const initialCachesRef = useRef(
    loadOutputCaches(maxCachedEvents, maxCachedSessions)
  );
  const [outputCache, setOutputCache] = useState<
    Record<string, OutputLine[]>
  >(initialCachesRef.current.outputCache);
  const [acpOutputCache, setAcpOutputCache] = useState<
    Record<string, OutputLine[]>
  >(initialCachesRef.current.acpOutputCache);
  const outputsRef = useRef(outputs);
  const acpOutputsRef = useRef(acpOutputs);
  const outputCacheRef = useRef(outputCache);
  const acpOutputCacheRef = useRef(acpOutputCache);
  const outputsKeyRef = useRef<string | null>(null);
  const loadSeq = useRef(0);
  const isComposingRef = useRef(false);
  const [showCreateAgent, setShowCreateAgent] = useState(false);
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>(
    []
  );
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const ansi = useMemo(() => createAnsiRenderer(), []);
  const [input, setInput] = useState("");
  const [inputHistory, setInputHistory] = useState<string[]>(() =>
    parseInputHistory(getLocalStorageItemSafe(INPUT_HISTORY_STORAGE_KEY))
  );
  const [inputHistoryCursor, setInputHistoryCursor] = useState(-1);
  const inputHistoryDraftRef = useRef("");
  const sseRef = useRef<EventSource | null>(null);
  const terminalRef = useRef<HTMLDivElement | null>(null);
  const terminalStickToBottomRef = useRef(true);
  const [eventMeta, setEventMeta] = useState<
    Record<
      string,
      {
        oldestId: number | null;
        hasMore: boolean;
        loading: boolean;
        loaded: boolean;
      }
    >
  >({});
  const [agentsCollapsed, setAgentsCollapsed] = useState(true);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [acpTab, setAcpTab] = useState<"conversation" | "debug">(
    "conversation"
  );
  const handleAcpTabSelect = useCallback((next: "conversation" | "debug") => {
    setAcpTab(next);
  }, []);
  const [createAgentBusy, setCreateAgentBusy] = useState(false);
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<
    AcpPermissionRecord[]
  >([]);
  const [pendingPermissionJump, setPendingPermissionJump] = useState<
    PendingPermissionJumpState | null
  >(null);
  const appRootRef = useRef<HTMLDivElement | null>(null);
  const appHeaderRef = useRef<HTMLElement | null>(null);
  const workspaceRef = useRef<HTMLElement | null>(null);
  const [, setThinkingTick] = useState(0);
  const createAgentBusyRef = useRef(false);
  const lastEventCursorRef = useRef<Record<string, EventCursor>>({});
  const eventPollRef = useRef<{
    timer: number | null;
    idleCount: number;
    boostUntil: number | null;
  }>({
    timer: null,
    idleCount: 0,
    boostUntil: null,
  });
  const schedulePollRef = useRef<((delay: number) => void) | null>(null);
  const permissionPollRef = useRef<{
    timer: number | null;
  }>({
    timer: null,
  });
  const schedulePermissionPollRef = useRef<((delay: number) => void) | null>(
    null
  );
  const outputPersistTimerRef = useRef<number | null>(null);
  const activeAgentRef = useRef<string | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const activeAgentStatusRef = useRef<string | null>(null);
  useEffect(() => {
    outputsRef.current = outputs;
  }, [outputs]);
  useEffect(() => {
    acpOutputsRef.current = acpOutputs;
  }, [acpOutputs]);
  useEffect(() => {
    outputCacheRef.current = outputCache;
  }, [outputCache]);
  useEffect(() => {
    acpOutputCacheRef.current = acpOutputCache;
  }, [acpOutputCache]);
  useEffect(() => {
    setLocalStorageItemSafe(
      INPUT_HISTORY_STORAGE_KEY,
      JSON.stringify(inputHistory)
    );
  }, [inputHistory]);
  const normalizedError = useMemo(() => {
    if (!error) return null;
    const message = sanitizeErrorBannerMessage(error, networkOnline);
    return shouldHideErrorBannerMessage(message) ? null : message;
  }, [error, networkOnline]);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    return setupRuntimeViewportVarSync(window, document.documentElement.style);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    return setupLayoutAnchorVarSync(
      window,
      document.documentElement.style,
      {
        appRoot: appRootRef.current,
        appHeader: appHeaderRef.current,
        workspace: workspaceRef.current,
      },
      typeof ResizeObserver === "undefined" ? undefined : ResizeObserver
    );
  }, [auth, normalizedError, agentsCollapsed]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onOnline = () => {
      setNetworkOnline(true);
      setError((prev) =>
        prev === OFFLINE_MESSAGE || prev === UPSTREAM_HTML_MESSAGE ? null : prev
      );
    };
    const onOffline = () => {
      setNetworkOnline(false);
      setSseState((prev) => (prev === "idle" ? "idle" : "reconnecting"));
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
  }, []);
  const updateOutputCacheEntry = useCallback(
    (key: string, ordered: OutputLine[]) => {
      const existing = outputCacheRef.current[key] ?? [];
      const nextSlice = buildOutputCacheSlice(
        existing,
        ordered,
        maxCachedEvents
      );
      if (!isSameOutputList(existing, nextSlice)) {
        const nextCache = { ...outputCacheRef.current, [key]: nextSlice };
        outputCacheRef.current = nextCache;
        setOutputCache(nextCache);
      }
      return nextSlice;
    },
    [maxCachedEvents]
  );
  const updateAcpOutputCacheEntry = useCallback(
    (key: string, ordered: OutputLine[]) => {
      const existing = acpOutputCacheRef.current[key] ?? [];
      const nextSlice = buildAcpCacheSlice(existing, ordered, maxCachedEvents);
      if (!isSameOutputList(existing, nextSlice)) {
        const nextCache = { ...acpOutputCacheRef.current, [key]: nextSlice };
        acpOutputCacheRef.current = nextCache;
        setAcpOutputCache(nextCache);
      }
      return nextSlice;
    },
    [maxCachedEvents]
  );
  const acpView = useMemo(() => buildAcpView(acpOutputs), [acpOutputs]);
  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );
  const activeAgentModelLabel = useMemo(() => {
    if (!activeAgentRecord) return null;
    return formatAgentModelLabel(
      activeAgentRecord.command,
      activeAgentRecord.args
    );
  }, [activeAgentRecord]);
  const scopedAcpPermissions = useMemo(() => {
    return filterPermissionsForAgent(acpPermissions, activeAgent);
  }, [acpPermissions, activeAgent]);
  const scopedAcpPermissionHistory = useMemo(() => {
    return filterPermissionsForAgent(acpPermissionHistory, activeAgent);
  }, [acpPermissionHistory, activeAgent]);
  const activeAgentStatus = activeAgentRecord?.status ?? null;
  const isAgentActive = isAgentActiveStatus(activeAgentStatus);
  const streamAgentIds = useMemo(() => buildSseTargetAgentIds(agents), [agents]);
  const streamAgentIdsQuery = useMemo(
    () => encodeSseTargetAgentIds(streamAgentIds),
    [streamAgentIds]
  );
  const hasSseTarget = streamAgentIds.length > 0;
  const connectionBadge = useMemo(
    () => deriveConnectionBadge(networkOnline, hasSseTarget, sseState),
    [networkOnline, hasSseTarget, sseState]
  );
  const thinkingStartTs =
    activeAgentStatus === "running" ? acpView.thinkingStartTs : null;
  const canControlAcp = Boolean(activeAgent && isAgentActive);
  const hasInProgressToolCall = acpView.toolCalls.some(
    (call) => call.status === "in_progress"
  );
  const canInterruptAcpRun =
    canControlAcp &&
    (acpView.runStatus?.status === "running" || hasInProgressToolCall);
  const activeEventKey = activeAgent
    ? `${activeAgent}:${activeSessionId ?? "latest"}`
    : null;
  const isOutputLoading =
    Boolean(activeEventKey) && eventMeta[activeEventKey]?.loaded !== true;

  const token = auth?.token ?? null;
  useEffect(() => {
    activeAgentRef.current = activeAgent;
  }, [activeAgent]);
  useEffect(() => {
    activeSessionIdRef.current = activeSessionId;
  }, [activeSessionId]);
  useEffect(() => {
    activeAgentStatusRef.current = activeAgentStatus;
  }, [activeAgentStatus]);
  useEffect(() => {
    setAcpPermissions([]);
    setAcpPermissionHistory([]);
    setPendingPermissionJump(null);
  }, [activeAgent]);

  useEffect(() => {
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, [activeAgent, activeSessionId]);

  const refreshAgents = useCallback(async () => {
    if (!token) return;
    try {
      const items = await api.listAgents(token);
      setAgents(items);
    } catch (err) {
      setError(formatWorktreeError(err) ?? String(err));
    }
  }, [token]);

  useEffect(() => {
    if (!token) return;
    refreshAgents();
  }, [token, refreshAgents]);

  useEffect(() => {
    if (activeAgent || agents.length === 0) return;
    const running = agents.find((agent) => isAgentActiveStatus(agent.status));
    const next = running ?? agents[0];
    if (next) {
      setActiveAgent(next.id);
      setActiveSessionId(agentSessions[next.id] ?? null);
    }
  }, [agents, activeAgent, agentSessions]);

  useEffect(() => {
    api.authStatus().then((res) => setRootInitialized(res.root_initialized)).catch(() => {});
  }, []);

  const loadAgentEvents = useCallback(async (
    id: string,
    sessionId?: string | null
  ): Promise<boolean> => {
    if (!token) return false;
    const seq = ++loadSeq.current;
    const key = `${id}:${sessionId ?? "latest"}`;
    setEventMeta((prev) => {
      const current = prev[key];
      if (current?.loading) return prev;
      return {
        ...prev,
        [key]: {
          oldestId: current?.oldestId ?? null,
          hasMore: current?.hasMore ?? false,
          loading: true,
          loaded: current?.loaded ?? false,
        },
      };
    });
    try {
      const events = await api.listAgentEvents(
        token,
        id,
        eventLimit,
        sessionId ?? undefined
      );
      if (seq !== loadSeq.current) return false;
      if (!sessionId) {
        const latestSession = [...events]
          .reverse()
          .find((evt) => evt.session_id)?.session_id;
        if (latestSession) {
          setAgentSessions((prev) => ({ ...prev, [id]: latestSession }));
          if (activeAgentRef.current === id && !activeSessionIdRef.current) {
            setActiveSessionId(latestSession);
          }
          setEventMeta((prev) => ({
            ...prev,
            [key]: {
              oldestId: null,
              hasMore: false,
              loading: false,
              loaded: true,
            },
          }));
          return true;
        }
      }
      const ordered = [...events].sort((a, b) => compareEventOrder(a, b));
      const nextSlice = updateOutputCacheEntry(key, ordered);
      const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
      if (acpOrdered.length > 0) {
        updateAcpOutputCacheEntry(key, acpOrdered);
      }
      const oldestEvent = nextSlice.length ? nextSlice[0] : null;
      const oldestId =
        typeof oldestEvent?.event_id === "number" ? oldestEvent.event_id : null;
      let hasNew = false;
      const maxCursor = getMaxEventCursor(ordered);
      if (maxCursor != null) {
        const prevCursor = lastEventCursorRef.current[key];
        lastEventCursorRef.current[key] = maxCursor;
        hasNew = prevCursor == null ? true : isCursorNewer(prevCursor, maxCursor);
      }
      setEventMeta((prev) => {
        const nextMeta = {
          oldestId,
          hasMore: ordered.length >= eventLimit,
          loading: false,
          loaded: true,
        };
        const current = prev[key];
        if (
          current &&
          current.oldestId === nextMeta.oldestId &&
          current.hasMore === nextMeta.hasMore &&
          current.loading === nextMeta.loading &&
          current.loaded === nextMeta.loaded
        ) {
          return prev;
        }
        return { ...prev, [key]: nextMeta };
      });
      return hasNew;
    } catch {
      // ignore
      setEventMeta((prev) => {
        const current = prev[key];
        if (!current) return prev;
        return { ...prev, [key]: { ...current, loading: false, loaded: true } };
      });
      return false;
    }
  }, [
    token,
    eventLimit,
    updateOutputCacheEntry,
    updateAcpOutputCacheEntry,
  ]);

  const loadOlderEvents = useCallback(async () => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore || meta.oldestId == null) {
      return;
    }
    setEventMeta((prev) => ({
      ...prev,
      [key]: { ...meta, loading: true, loaded: true },
    }));
    try {
      const older = await api.listAgentEvents(
        token,
        activeAgent,
        eventLimit,
        activeSessionId ?? undefined,
        meta.oldestId
      );
      const ordered = [...older].sort((a, b) => compareEventOrder(a, b));
      const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
      const nextOldestEvent = ordered.length ? ordered[0] : null;
      const nextOldestId =
        typeof nextOldestEvent?.event_id === "number"
          ? nextOldestEvent.event_id
          : meta.oldestId;
      const hasMore = ordered.length >= eventLimit;
      setOutputs((prev) => mergeOutputs(prev, ordered));
      setAcpOutputs((prev) => mergeOutputs(prev, acpOrdered));
      updateOutputCacheEntry(key, ordered);
      if (acpOrdered.length > 0) {
        updateAcpOutputCacheEntry(key, acpOrdered);
      }
      setEventMeta((prev) => ({
        ...prev,
        [key]: {
          oldestId: nextOldestId ?? null,
          hasMore,
          loading: false,
          loaded: true,
        },
      }));
    } catch {
      setEventMeta((prev) => ({
        ...prev,
        [key]: { ...meta, loading: false, loaded: true },
      }));
    }
  }, [
    token,
    activeAgent,
    activeSessionId,
    eventMeta,
    eventLimit,
    updateOutputCacheEntry,
    updateAcpOutputCacheEntry,
  ]);

  const acpConversation = useAcpConversation({
    acpView,
    activeAgent,
    activeSessionId,
    acpTab,
    eventMeta,
    isAgentActive,
    onLoadOlder: loadOlderEvents,
  });
  useEffect(() => {
    if (outputPersistTimerRef.current) {
      window.clearTimeout(outputPersistTimerRef.current);
    }
    outputPersistTimerRef.current = window.setTimeout(() => {
      saveOutputCaches(
        outputCache,
        acpOutputCache,
        maxCachedEvents,
        maxCachedSessions
      );
    }, 500);
    return () => {
      if (outputPersistTimerRef.current) {
        window.clearTimeout(outputPersistTimerRef.current);
        outputPersistTimerRef.current = null;
      }
    };
  }, [outputCache, acpOutputCache, maxCachedEvents, maxCachedSessions]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const latestKey = `${activeAgent}:latest`;
    const previousKey = outputsKeyRef.current;
    const selection = selectCachedOutputs(
      outputCache,
      acpOutputCache,
      key,
      latestKey
    );
    if (selection.source === "none") {
      setOutputs([]);
      setAcpOutputs([]);
      outputsKeyRef.current = key;
      loadAgentEvents(activeAgent, activeSessionId);
      return;
    }
    const baseOutputs = selection.outputs ?? [];
    const baseAcpOutputs = selection.acpOutputs ?? [];
    const latestOutputs =
      activeSessionId && key !== latestKey
        ? (outputCache[latestKey] ?? []).filter(
            (evt) => evt.session_id === activeSessionId
          )
        : [];
    const latestAcpOutputs =
      activeSessionId && key !== latestKey
        ? (acpOutputCache[latestKey] ?? []).filter(
            (evt) => evt.session_id === activeSessionId
          )
        : [];
    const combinedOutputs =
      selection.source === "session" &&
      activeSessionId &&
      key !== latestKey &&
      baseOutputs.length > 0
        ? mergeOutputs(baseOutputs, latestOutputs)
        : baseOutputs;
    const combinedAcpOutputs =
      selection.source === "session" &&
      activeSessionId &&
      key !== latestKey &&
      baseAcpOutputs.length > 0
        ? mergeOutputs(baseAcpOutputs, latestAcpOutputs)
        : baseAcpOutputs;
    const shouldPreserveHistory = previousKey === key;
    const nextOutputs = mergeOutputsPreserveHistory(
      outputsRef.current,
      combinedOutputs,
      shouldPreserveHistory
    );
    const nextAcpOutputs = mergeOutputsPreserveHistory(
      acpOutputsRef.current,
      combinedAcpOutputs,
      shouldPreserveHistory
    );
    if (!isSameOutputList(outputsRef.current, nextOutputs)) {
      setOutputs(nextOutputs);
    }
    if (!isSameOutputList(acpOutputsRef.current, nextAcpOutputs)) {
      setAcpOutputs(nextAcpOutputs);
    }
    outputsKeyRef.current = key;
    if (!eventMeta[key]) {
      const oldestEvent = nextOutputs.length
        ? nextOutputs[0]
        : nextAcpOutputs.length
          ? nextAcpOutputs[0]
          : null;
      const oldestId =
        typeof oldestEvent?.event_id === "number" ? oldestEvent.event_id : null;
      setEventMeta((prev) => ({
        ...prev,
        [key]: {
          oldestId,
          hasMore:
            nextOutputs.length + nextAcpOutputs.length >= eventLimit,
          loading: false,
          loaded: true,
        },
      }));
    }
  }, [
    token,
    activeAgent,
    activeSessionId,
    outputCache,
    acpOutputCache,
    eventMeta,
    eventLimit,
    loadAgentEvents,
  ]);

  useEffect(() => {
    if (!token) {
      setDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
      setAgentWorkdir("");
      return;
    }
    api
      .getRuntimeDefaults(token)
      .then((defaults) => {
        // Backend guarantees a non-empty default root; keep a defensive fallback
        // in case of malformed responses.
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

  const handleWorktreeModeChange = useCallback(
    (nextMode: "use_existing" | "create_worktree" | "reuse_worktree") => {
      setWorktreeMode(nextMode);
      setAgentWorkdir((prev) =>
        resolveWorkdirForModeChange(
          prev,
          nextMode,
          defaultWorktreeRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
    },
    [defaultWorktreeRoot]
  );

  const openCreateAgentModal = useCallback(() => {
    setAgentWorkdir((prev) =>
      resolveWorkdirForModalOpen(
        prev,
        worktreeMode,
        defaultWorktreeRoot,
        DEFAULT_WORKTREE_ROOT
      )
    );
    setShowCreateAgent(true);
  }, [defaultWorktreeRoot, worktreeMode]);

  useEffect(() => {
    if (!token || auth?.role !== "root") return;
    api.listSafePaths(token).then(setSafePaths).catch(() => {});
    api.listDevices(token).then(setDevices).catch(() => {});
    api.listAudits(token).then(setAudits).catch(() => {});
    api.getVapidInfo(token).then(setVapidInfo).catch(() => {});
  }, [token, auth?.role]);

  useEffect(() => {
    setError((prev) => sanitizeAgentError(prev, activeAgentStatus));
  }, [activeAgentStatus]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    loadAgentEvents(activeAgent, activeSessionId);
  }, [token, activeAgent, activeSessionId, loadAgentEvents]);

  useEffect(() => {
    if (!token) {
      setSseState("idle");
      return;
    }
    const streamTarget = streamAgentIdsQuery;
    if (!hasSseTarget || streamTarget.length === 0) {
      setSseState("idle");
      return;
    }
    let cancelled = false;
    const pollState = eventPollRef.current;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;
    const clearReconnectTimer = () => {
      if (reconnectTimer) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
    };
    const pollActiveAgent = async (): Promise<boolean> => {
      const currentActive = activeAgentRef.current;
      if (!currentActive) return false;
      return (
        (await loadAgentEvents(currentActive, activeSessionIdRef.current)) === true
      );
    };
    const scheduleReconnect = () => {
      if (cancelled) return;
      clearReconnectTimer();
      setSseState("reconnecting");
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (cancelled) return;
        openSource();
      }, delay);
    };
    const openSource = () => {
      if (cancelled) return;
      setSseState("connecting");
      const source = new EventSource(
        `${location.origin}/sse/agents?ids=${encodeURIComponent(
          streamTarget
        )}&token=${encodeURIComponent(token)}`
      );
      sseRef.current = source;
      source.onopen = () => {
        reconnectAttempt = 0;
        setNetworkOnline(true);
        setSseState("connected");
        if (pollState.timer) {
          window.clearTimeout(pollState.timer);
          pollState.timer = null;
        }
      };
      source.onmessage = (event) => {
        if (event.data === "heartbeat") return;
        try {
          const parsed = JSON.parse(event.data);
          if (parsed.type === "output" || parsed.type === "acp") {
            const payload = parsed.payload;
            if (!isValidOutputPayload(payload)) {
              return;
            }
            const line: OutputLine = {
              event_id: payload.event_id,
              ts: payload.ts,
              stream: payload.stream,
              message: payload.message,
              agent_id: payload.agent_id,
              session_id: payload.session_id,
              seq: payload.seq,
            };
            const key = `${payload.agent_id}:${payload.session_id ?? "latest"}`;
            updateLastEventCursor(lastEventCursorRef, key, line);
            if (payload.stream === "acp") {
              const status = parseRunStatus(payload.message);
              if (status) {
                setAgents((prev) =>
                  prev.map((agent) =>
                    agent.id === payload.agent_id
                      ? { ...agent, status: statusToAgentStatus(status) }
                      : agent
                    )
                );
              }
              updateAcpOutputCacheEntry(key, [line]);
            }
            updateOutputCacheEntry(key, [line]);
            const currentActive = activeAgentRef.current;
            if (payload.agent_id !== currentActive) {
              return;
            }
            const currentSessionId = activeSessionIdRef.current;
            if (currentSessionId && payload.session_id !== currentSessionId) {
              return;
            }
            setOutputs((prev) => appendOutputLine(prev, line));
            if (line.stream === "acp") {
              setAcpOutputs((prev) => appendOutputLine(prev, line));
            }
          }
        } catch {
          if (typeof event.data === "string") {
            if (isInvalidTokenMessage(event.data)) {
              clearAuthAndRedirect();
              return;
            }
            const normalizedStreamError = sanitizeErrorBannerMessage(
              event.data,
              getNavigatorOnline()
            );
            if (
              shouldIgnoreAgentWsError(
                normalizedStreamError,
                activeAgentStatusRef.current
              )
            ) {
              return;
            }
            setError(normalizedStreamError);
          }
        }
      };
      source.onerror = () => {
        if (sseRef.current !== source) {
          source.close();
          return;
        }
        const online = getNavigatorOnline();
        setNetworkOnline(online);
        setSseState("reconnecting");
        setError(
          online
            ? UPSTREAM_HTML_MESSAGE
            : OFFLINE_MESSAGE
        );
        source.close();
        sseRef.current = null;
        schedulePoll(getAdaptivePollInterval(pollState.idleCount));
        scheduleReconnect();
      };
    };
    openSource();
    const schedulePoll = (delay: number) => {
      if (cancelled) return;
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
      }
      if (sseRef.current?.readyState === EventSource.OPEN) {
        pollState.timer = null;
        return;
      }
      const now = Date.now();
      const boostUntil = pollState.boostUntil;
      const boostActive = boostUntil != null && boostUntil > now;
      const nextDelay = boostActive ? 1000 : delay;
      pollState.timer = window.setTimeout(async () => {
        if (cancelled) return;
        const current = sseRef.current;
        const isOpen =
          current != null && current.readyState === EventSource.OPEN;
        let hasNew = false;
        if (!isOpen) {
          hasNew = (await pollActiveAgent()) === true;
        } else {
          pollState.idleCount = 0;
        }
        if (hasNew) {
          pollState.idleCount = 0;
        } else if (!isOpen) {
          pollState.idleCount += 1;
        }
        if (!cancelled) {
          schedulePoll(getAdaptivePollInterval(pollState.idleCount));
        }
      }, nextDelay);
    };
    schedulePollRef.current = schedulePoll;
    schedulePoll(getAdaptivePollInterval(0));
    return () => {
      cancelled = true;
      clearReconnectTimer();
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
        pollState.timer = null;
      }
      pollState.idleCount = 0;
      pollState.boostUntil = null;
      schedulePollRef.current = null;
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
      setSseState("idle");
    };
  }, [
    token,
    hasSseTarget,
    streamAgentIdsQuery,
    loadAgentEvents,
    updateOutputCacheEntry,
    updateAcpOutputCacheEntry,
  ]);

  useEffect(() => {
    if (acpView.hasAcp) return;
    const el = terminalRef.current;
    if (!el) return;
    if (terminalStickToBottomRef.current) {
      el.scrollTop = el.scrollHeight;
      return;
    }
  }, [outputs.length, acpView.hasAcp]);

  useEffect(() => {
    terminalStickToBottomRef.current = true;
  }, [activeAgent, activeSessionId]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    let cancelled = false;
    const requestedAgentId = activeAgent;
    const pollState = permissionPollRef.current;
    const pollOnce = async (): Promise<number> => {
      try {
        const items = await api.listAcpPermissions(
          token,
          requestedAgentId,
          "pending"
        );
        if (!cancelled) {
          if (activeAgentRef.current !== requestedAgentId) return 0;
          setAcpPermissions((prev) => (isSamePermissionList(prev, items) ? prev : items));
        }
        return items.length;
      } catch {
        if (!cancelled) setAcpPermissions([]);
        return 0;
      }
    };
    const schedule = (delay: number) => {
      schedulePermissionPollLoop(
        delay,
        pollState,
        pollOnce,
        () => cancelled
      );
    };
    schedulePermissionPollRef.current = schedule;
    schedule(0);
    return () => {
      cancelled = true;
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
        pollState.timer = null;
      }
      schedulePermissionPollRef.current = null;
    };
  }, [token, activeAgent, activeSessionId]);

  useEffect(() => {
    if (!thinkingStartTs) return;
    if (activeAgentStatus !== "running") return;
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [thinkingStartTs, activeAgentStatus]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    let cancelled = false;
    const requestedAgentId = activeAgent;
    const load = async () => {
      try {
        const items = await api.listAcpPermissions(token, requestedAgentId);
        if (!cancelled) {
          if (activeAgentRef.current !== requestedAgentId) return;
          setAcpPermissionHistory((prev) => (isSamePermissionList(prev, items) ? prev : items));
        }
      } catch {
        if (!cancelled) setAcpPermissionHistory([]);
      }
    };
    load();
    const timer = window.setInterval(load, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [token, activeAgent, activeSessionId]);

  const onRegister = async (role?: string) => {
    setError(null);
    try {
      const start = await api.registerStart(
        username,
        displayName,
        role,
        role === "root" ? password : undefined
      );
      const options = publicKeyCredentialCreationOptionsFromJson(start.options);
      const cred = await navigator.credentials.create({ publicKey: options });
      if (!cred) throw new Error("registration cancelled");
      const payload = registerCredentialToJson(cred as PublicKeyCredential);
      const finish = await api.registerFinish(start.challenge_id, payload);
      const next = {
        token: finish.token,
        userId: finish.user_id,
        username,
        role: finish.role,
      };
      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(finish.token);
    } catch (err) {
      setError(formatWorktreeError(err) ?? String(err));
    }
  };

  const onLogin = async () => {
    setError(null);
    try {
      const start = await api.loginStart(username, password);
      const options = publicKeyCredentialRequestOptionsFromJson(start.options);
      const cred = await navigator.credentials.get({ publicKey: options });
      if (!cred) throw new Error("login cancelled");
      const payload = loginCredentialToJson(cred as PublicKeyCredential);
      const finish = await api.loginFinish(start.challenge_id, payload);
      const next = {
        token: finish.token,
        userId: finish.user_id,
        username,
        role: finish.role,
      };
      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(finish.token);
    } catch (err) {
      setError(String(err));
    }
  };

  const onLogout = () => {
    removeLocalStorageItemSafe("agenthub_auth");
    setAuth(null);
    setAgents([]);
    setActiveAgent(null);
    setOutputs([]);
    setAcpOutputs([]);
    setSafePaths([]);
    setDevices([]);
    setAcpPermissions([]);
    setVapidInfo(null);
    setWorktreeError(null);
    setDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
    setAgentWorkdir("");
  };

  const onCreateAgent = async () => {
    if (!token) return;
    if (createAgentBusyRef.current) return;
    createAgentBusyRef.current = true;
    setCreateAgentBusy(true);
    setError(null);
    setWorktreeError(null);
    try {
      const name = agentName.trim() || "agent";
      const workdir = normalizeWorkdirInput(agentWorkdir);
      const normalizedRoot = normalizeWorkdirInput(defaultWorktreeRoot);
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
        workdir: workdirPayload,
        command,
        args,
        worktree_mode: worktreeMode,
        worktree_repo: worktreeRepo.trim() || null,
        worktree_ref: worktreeRef.trim() || null,
        code_mode: codeMode,
      });
      setAgents((prev) => [agent, ...prev]);
      try {
        const res = await api.startAgent(token, agent.id);
        setActiveSessionId(res.session_id);
        setAgentSessions((prev) => ({ ...prev, [agent.id]: res.session_id }));
        setActiveAgent(agent.id);
        await refreshAgents();
      } catch (err) {
        const msg = parseApiErrorMessage(err) ?? String(err);
        setError(`Start failed: ${msg}`);
      }
      setAgentName("");
      setAgentWorkdir("");
      setAgentPresetId(DEFAULT_AGENT_PRESET_ID);
      setWorktreeMode("use_existing");
      setWorktreeRepo("");
      setWorktreeRef("");
      setCodeMode(true);
      setShowCreateAgent(false);
    } catch (err) {
      const hint = formatWorktreeError(err);
      if (hint) {
        setWorktreeError(hint);
      }
      setError(hint ?? parseApiErrorMessage(err) ?? String(err));
    } finally {
      createAgentBusyRef.current = false;
      setCreateAgentBusy(false);
    }
  };

  const onStartAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    setWorktreeError(null);
    try {
      const res = await api.startAgent(token, id);
      setActiveSessionId(res.session_id);
      setAgentSessions((prev) => ({ ...prev, [id]: res.session_id }));
      setActiveAgent(id);
      await refreshAgents();
    } catch (err) {
      const message = parseApiErrorMessage(err) ?? String(err);
      if (message.toLowerCase().includes("agent already running")) {
        await refreshAgents();
        return;
      }
      const hint = formatWorktreeError(err);
      setError(hint ?? message);
    }
  }, [token, refreshAgents]);

  const onStopAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    setWorktreeError(null);
    try {
      await api.stopAgent(token, id);
      setActiveSessionId(null);
      await refreshAgents();
    } catch (err) {
      setError(String(err));
    }
  }, [token, refreshAgents]);

  const onDeleteAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    try {
      await api.deleteAgent(token, id);
      setAgents((prev) => prev.filter((agent) => agent.id !== id));
      setAgentSessions((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      if (activeAgent === id) {
        setActiveAgent(null);
        setActiveSessionId(null);
        setOutputs([]);
        setAcpOutputs([]);
      }
    } catch (err) {
      setError(String(err));
    }
  }, [token, activeAgent]);

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
    } catch (err) {
      setError(String(err));
    }
  }, [token]);

  const onAcpSetMode = useCallback(async () => {
    if (!token || !activeAgent) return;
    const modeId = acpModeId.trim();
    if (!modeId) {
      setError("mode id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpMode(token, activeAgent, modeId);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, acpModeId]);

  const onAcpSetModel = useCallback(async () => {
    if (!token || !activeAgent) return;
    const modelId = acpModelId.trim();
    if (!modelId) {
      setError("model id is required");
      return;
    }
    setError(null);
    try {
      await api.setAcpModel(token, activeAgent, modelId);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, acpModelId]);

  const onAcpSetConfig = useCallback(async () => {
    if (!token || !activeAgent) return;
    const configId = acpConfigId.trim();
    const configValue = acpConfigValue.trim();
    if (!configId || !configValue) {
      setError("config id and value are required");
      return;
    }
    setError(null);
    try {
      await api.setAcpConfig(token, activeAgent, configId, configValue);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent, acpConfigId, acpConfigValue]);

  const onAcpCancel = useCallback(async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.cancelAcp(token, activeAgent);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent]);

  const onAcpClearSession = useCallback(async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.clearAcpSession(token, activeAgent);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token, activeAgent]);

  const onSendInput = useCallback(async () => {
    if (!input.trim()) return;
    if (!token || !activeAgent) return;
    eventPollRef.current.boostUntil = Date.now() + 10_000;
    schedulePollRef.current?.(1000);
    acpConversation.jumpToConversationBottom();
    const text = input.trim();
    let messageId: string | null = null;
    if (activeSessionId) {
      messageId =
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `local-${Date.now()}`;
    }
    try {
      await api.sendInput(token, activeAgent, text, messageId ?? undefined);
      setInputHistory((prev) => pushInputHistory(prev, text));
      setInputHistoryCursor(-1);
      inputHistoryDraftRef.current = "";
      setInput("");
    } catch (err) {
      const msg = String(err || "websocket not connected");
      setError(msg);
      if (msg.includes(AGENT_NOT_RUNNING_ERROR)) {
        await refreshAgents();
      }
    }
  }, [
    input,
    token,
    activeAgent,
    activeSessionId,
    acpConversation.jumpToConversationBottom,
    refreshAgents,
  ]);

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

  const handleCollapseAgents = useCallback(() => {
    setAgentsCollapsed(true);
  }, []);

  const handleExpandAgents = useCallback(() => {
    setAgentsCollapsed(false);
  }, []);

  const handleToggleAgents = useCallback(() => {
    setAgentsCollapsed((prev) => !prev);
  }, []);

  const handleSelectAgent = useCallback((id: string) => {
    setActiveAgent(id);
    setActiveSessionId(agentSessions[id] ?? null);
    setAgentsCollapsed(true);
  }, [agentSessions]);

  const onRespondPermission = async (
    agentId: string,
    permissionId: string,
    optionId?: string
  ) => {
    if (!token) return;
    setPermissionBusy(permissionId);
    schedulePermissionPollRef.current?.(1000);
    try {
      await api.respondAcpPermission(token, agentId, permissionId, {
        option_id: optionId ?? null,
        outcome: optionId ? undefined : "cancelled",
      });
      setAcpPermissions((prev) =>
        prev.filter((item) => item.id !== permissionId)
      );
    } catch (err) {
      setError(String(err));
    } finally {
      setPermissionBusy(null);
    }
  };

  const onCreateJoin = async () => {
    if (!token) return;
    setError(null);
    try {
      const data = await api.joinStartAdmin(token);
      setJoinPin(data.pin);
      setJoinToken(data.token);
      const url = `${location.origin}/join?token=${data.token}`;
      const { toDataURL } = await import("qrcode");
      const qr = await toDataURL(url);
      setJoinQr(qr);
    } catch (err) {
      setError(String(err));
    }
  };

  const onAddSafePath = async () => {
    if (!token) return;
    try {
      const path = safePathInput.trim();
      if (!path) return;
      await api.addSafePath(token, path);
      const list = await api.listSafePaths(token);
      setSafePaths(list);
      setSafePathInput("");
    } catch (err) {
      setError(String(err));
    }
  };

  const onDeleteSafePath = async (path: string) => {
    if (!token) return;
    setError(null);
    try {
      await api.deleteSafePath(token, path);
      const list = await api.listSafePaths(token);
      setSafePaths(list);
    } catch (err) {
      setError(String(err));
    }
  };

  const onToggleSafePath = (path: string) => {
    setSelectedSafePaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  };

  const onToggleAllSafePaths = () => {
    if (selectedSafePaths.size === safePaths.length) {
      setSelectedSafePaths(new Set());
      return;
    }
    setSelectedSafePaths(new Set(safePaths.map((p) => p.path)));
  };

  const onDeleteSelectedSafePaths = async () => {
    if (!token) return;
    setError(null);
    try {
      for (const path of selectedSafePaths) {
        await api.deleteSafePath(token, path);
      }
      const list = await api.listSafePaths(token);
      setSafePaths(list);
      setSelectedSafePaths(new Set());
    } catch (err) {
      setError(String(err));
    }
  };

  const onRevokeDevice = async (id: string) => {
    if (!token) return;
    setError(null);
    try {
      await api.revokeDevice(token, id);
      const list = await api.listDevices(token);
      setDevices(list);
      const items = await api.listAudits(token);
      setAudits(items);
    } catch (err) {
      setError(String(err));
    }
  };

  const onRotateVapid = async () => {
    if (!token) return;
    setError(null);
    try {
      await api.rotateVapid(token);
      const info = await api.getVapidInfo(token);
      setVapidInfo(info);
    } catch (err) {
      setError(String(err));
    }
  };

  const acpRuntimeMetrics = useMemo(() => {
    const cacheStats = getAcpConversationCacheStats();
    return {
      totalConversationItems: acpConversation.conversationTotalItems,
      sourceConversationItems: acpConversation.conversationSourceItems,
      renderedConversationItems: acpConversation.conversationRenderedItems,
      pendingConversationItems: acpConversation.conversationPendingCount,
      virtualizedConversation: acpConversation.conversationVirtualized,
      stickToBottom: acpConversation.conversationStickToBottom,
      averageConversationHeight: Math.round(acpConversation.conversationAvgHeight),
      rawEventCount: acpView.rawEvents.length,
      toolCallCount: acpView.toolCalls.length,
      messageCount: acpView.messages.length,
      markdownCacheHits: cacheStats.markdownHits,
      markdownCacheMisses: cacheStats.markdownMisses,
      ansiCacheHits: cacheStats.ansiHits,
      ansiCacheMisses: cacheStats.ansiMisses,
      payloadParses: cacheStats.payloadParses,
      payloadParseFailures: cacheStats.payloadParseFailures,
    };
  }, [
    acpConversation.conversationTotalItems,
    acpConversation.conversationSourceItems,
    acpConversation.conversationRenderItems,
    acpConversation.conversationPendingCount,
    acpConversation.conversationVirtualized,
    acpConversation.conversationStickToBottom,
    acpConversation.conversationAvgHeight,
    acpView.rawEvents.length,
    acpView.toolCalls.length,
    acpView.messages.length,
  ]);

  const onJumpToPermissionHistory = useCallback(
    (permission: AcpPermissionRecord) => {
      const toolCallId = permission.tool_call_id?.trim();
      if (!toolCallId) return;
      setAcpTab("conversation");
      const targetSessionId = permission.session_id ?? null;
      if (targetSessionId && targetSessionId !== activeSessionId) {
        setActiveSessionId(targetSessionId);
      }
      setPendingPermissionJump({
        toolCallId,
        sessionId: targetSessionId,
        attempts: 0,
      });
    },
    [activeSessionId]
  );

  useEffect(() => {
    const jumpDecision = decidePermissionJump(
      pendingPermissionJump,
      acpTab,
      activeSessionId
    );
    if (jumpDecision === "idle" || jumpDecision === "wait") return;
    if (jumpDecision === "clear") {
      setPendingPermissionJump(null);
      return;
    }
    if (!pendingPermissionJump) return;
    if (acpConversation.jumpToConversationToolCall(pendingPermissionJump.toolCallId)) {
      setPendingPermissionJump(null);
      return;
    }
    const timer = window.setTimeout(() => {
      setPendingPermissionJump((prev) => {
        if (!prev) return prev;
        return { ...prev, attempts: prev.attempts + 1 };
      });
    }, PERMISSION_JUMP_RETRY_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [
    pendingPermissionJump,
    acpTab,
    activeSessionId,
    acpConversation.jumpToConversationToolCall,
  ]);

  const acpConversationProps = useMemo(
    () => ({
      items: acpConversation.conversationRenderItems,
      windowOffset: acpConversation.conversationWindowOffset,
      isFrozenView: acpConversation.isFrozenView,
      shouldAutoCollapse: acpConversation.shouldAutoCollapse,
      collapseCutoff: acpConversation.collapseCutoff,
      runStatus: acpView.runStatus?.status ?? null,
      virtualTopSpacer: acpConversation.conversationVirtualTopSpacer,
      virtualBottomSpacer: acpConversation.conversationVirtualBottomSpacer,
      stickToBottom: acpConversation.conversationStickToBottom,
      pendingCount: acpConversation.conversationPendingCount,
      avgHeight: acpConversation.conversationAvgHeight,
      focusedToolCallId: acpConversation.focusedConversationToolCallId,
      onScroll: acpConversation.handleConversationScroll,
      containerRef: acpConversation.acpConversationRef,
      ansi,
    }),
    [
      acpConversation.conversationRenderItems,
      acpConversation.conversationWindowOffset,
      acpConversation.isFrozenView,
      acpConversation.shouldAutoCollapse,
      acpConversation.collapseCutoff,
      acpConversation.conversationVirtualTopSpacer,
      acpConversation.conversationVirtualBottomSpacer,
      acpConversation.conversationStickToBottom,
      acpConversation.conversationPendingCount,
      acpConversation.conversationAvgHeight,
      acpConversation.focusedConversationToolCallId,
      acpConversation.handleConversationScroll,
      acpConversation.acpConversationRef,
      acpView.runStatus?.status,
      ansi,
    ]
  );
  const acpDebugProps = useMemo(
    () => ({
      currentMode: acpView.currentMode,
      rawEvents: acpView.rawEvents,
      acpPermissionHistory: scopedAcpPermissionHistory,
      acpModeId,
      acpModelId,
      acpConfigId,
      acpConfigValue,
      onAcpModeIdChange: setAcpModeId,
      onAcpModelIdChange: setAcpModelId,
      onAcpConfigIdChange: setAcpConfigId,
      onAcpConfigValueChange: setAcpConfigValue,
      canControlAcp,
      onAcpSetMode,
      onAcpSetModel,
      onAcpSetConfig,
      onAcpCancel,
      onAcpClearSession,
      onJumpToPermissionHistory,
      runtimeMetrics: acpRuntimeMetrics,
    }),
    [
      acpView.currentMode,
      acpView.rawEvents,
      scopedAcpPermissionHistory,
      acpModeId,
      acpModelId,
      acpConfigId,
      acpConfigValue,
      canControlAcp,
      onAcpSetMode,
      onAcpSetModel,
      onAcpSetConfig,
      onAcpCancel,
      onAcpClearSession,
      onJumpToPermissionHistory,
      acpRuntimeMetrics,
    ]
  );
  const acpPanelProps = useMemo(
    () => ({
      acpView,
      subtitle: activeAgentRecord?.workdir ?? null,
      acpTab,
      onSelectTab: handleAcpTabSelect,
      showConversationBadge: acpConversation.showConversationBadge,
      conversation: acpConversationProps,
      debug: acpDebugProps,
    }),
    [
      acpView,
      activeAgentRecord?.workdir,
      acpTab,
      handleAcpTabSelect,
      acpConversation.showConversationBadge,
      acpConversationProps,
      acpDebugProps,
    ]
  );
  const showInputDock = !(acpTab === "debug" && acpView.hasAcp);

  if (location.pathname.startsWith("/join")) {
    return <JoinPage onComplete={(next) => setAuth(next)} />;
  }

  if (location.pathname.startsWith("/admin")) {
    if (!auth) {
      return <AuthRequired />;
    }
    if (auth.role !== "root") {
      return <ForbiddenPage />;
    }
    return (
      <AdminPage
        auth={auth}
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
        joinQr={joinQr}
        joinToken={joinToken}
        joinPin={joinPin}
        safePathInput={safePathInput}
        setSafePathInput={setSafePathInput}
      />
    );
  }

  if (location.pathname.startsWith("/teams")) {
    if (!auth || !token) {
      return <AuthRequired />;
    }
    return <TeamPage auth={auth} token={token} onLogout={onLogout} />;
  }

  return (
    <div className="app" ref={appRootRef}>
      <header ref={appHeaderRef}>
        <h1>AgentHub</h1>
        {auth && (
          <div className="session">
            <span
              className={`session-connection ${connectionBadge.tone}`}
              title={connectionBadge.title}
              role="status"
              aria-live="polite"
            >
              <span className="session-connection-dot" aria-hidden="true" />
              <span>{connectionBadge.label}</span>
            </span>
            <span>{auth.username}</span>
            <a
              className="icon-button"
              href="/teams"
              title="Teams"
              aria-label="Teams"
            >
              <i className="bi bi-diagram-3" aria-hidden="true" />
            </a>
            {auth.role === "root" && (
              <a
                className="icon-button"
                href="/admin"
                title="Admin"
                aria-label="Admin"
              >
                <i className="bi bi-gear" aria-hidden="true" />
              </a>
            )}
            <button onClick={onLogout}>Logout</button>
          </div>
        )}
      </header>

      {normalizedError && (
        <ErrorBanner message={normalizedError} onClose={() => setError(null)} />
      )}

      {!auth && (
        <section className="auth">
          <h2>Password + Passkey Login</h2>
          <input
            placeholder="Username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
          />
          <input
            placeholder="Password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
          {rootInitialized === false && (
            <input
              placeholder="Display Name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
            />
          )}
          <div className="actions">
            {rootInitialized === false && (
              <button onClick={() => onRegister("root")}>Bootstrap Root</button>
            )}
            <button onClick={onLogin}>Login</button>
          </div>
        </section>
      )}

      {auth && (
        <section
          className={agentsCollapsed ? "workspace collapsed" : "workspace"}
          ref={workspaceRef}
        >
          <AgentsPanel
            agents={agents}
            activeAgent={activeAgent}
            agentsCollapsed={agentsCollapsed}
            onCollapse={handleCollapseAgents}
            onExpand={handleExpandAgents}
            onCreateAgent={openCreateAgentModal}
            onSelectAgent={handleSelectAgent}
            onToggleCodeMode={onSetCodeMode}
            onStartAgent={onStartAgent}
            onStopAgent={onStopAgent}
            onDeleteAgent={onDeleteAgent}
          />
          <div className="workspace-right">
            <OutputHeader
              activeAgent={activeAgentRecord}
              activeSessionId={activeSessionId}
              agentsCollapsed={agentsCollapsed}
              hasAcp={acpView.hasAcp}
              thinkingStartTs={thinkingStartTs}
              modelLabel={activeAgentModelLabel}
              onToggleAgents={handleToggleAgents}
            />
            {activeAgent ? (
              <OutputErrorBoundary>
                <OutputBody
                  terminalRef={terminalRef}
                  onTerminalScroll={handleTerminalScroll}
                  isOutputLoading={isOutputLoading}
                  outputs={outputs}
                  ansi={ansi}
                  acpPanelProps={acpPanelProps}
                />
              </OutputErrorBoundary>
            ) : null}
            {showInputDock && (
              <InputDock
                input={input}
                historyCommands={inputHistory}
                showInterrupt={acpView.hasAcp}
                canInterrupt={canInterruptAcpRun}
                onInputChange={onInputChange}
                onSendInput={onSendInput}
                onInterrupt={onAcpCancel}
                onNavigateHistory={onNavigateInputHistory}
                onSelectHistoryCommand={onSelectInputHistory}
                onJumpToBottom={acpConversation.jumpToConversationBottom}
                showConversationJump={acpConversation.showConversationJump}
                isComposingRef={isComposingRef}
              />
            )}
          </div>
        </section>
      )}

      {auth && showCreateAgent && (
        <CreateAgentModal
          agentName={agentName}
          setAgentName={setAgentName}
          agentWorkdir={agentWorkdir}
          setAgentWorkdir={setAgentWorkdir}
          agentPresetId={agentPresetId}
          setAgentPresetId={setAgentPresetId}
          worktreeMode={worktreeMode}
          setWorktreeMode={handleWorktreeModeChange}
          worktreeRepo={worktreeRepo}
          setWorktreeRepo={setWorktreeRepo}
          worktreeRef={worktreeRef}
          setWorktreeRef={setWorktreeRef}
          codeMode={codeMode}
          setCodeMode={setCodeMode}
          worktreeError={worktreeError}
          createBusy={createAgentBusy}
          workdirPlaceholder={defaultWorktreeRoot}
          onCreateAgent={onCreateAgent}
          onClose={() => setShowCreateAgent(false)}
        />
      )}

      {auth && activeAgent && scopedAcpPermissions.length > 0 && (
        <PermissionModal
          permissions={scopedAcpPermissions}
          permissionBusy={permissionBusy}
          onRespond={onRespondPermission}
        />
      )}
    </div>
  );
}

function formatWorktreeError(err: unknown): string | null {
  const msg = parseApiErrorMessage(err);
  if (!msg) return null;
  const lower = msg.toLowerCase();
  if (!lower.includes("worktree") && !lower.includes("workdir")) return null;
  if (lower.includes("workdir not allowed")) {
    return "Workdir not allowed. Add the path to Safe Paths before starting the agent.";
  }
  if (lower.includes("worktree_repo required")) {
    return "Worktree repo is required for the selected mode.";
  }
  if (lower.includes("worktree does not exist")) {
    return "Worktree does not exist. Use Create Worktree or choose an existing workdir.";
  }
  if (lower.includes("workdir is not empty")) {
    return "Workdir is not empty. Choose an empty directory for Create Worktree.";
  }
  if (lower.includes("git worktree add failed")) {
    return `Git worktree add failed. ${msg}`;
  }
  return msg;
}

function parseApiErrorMessage(err: unknown): string | null {
  if (!err) return null;
  if (typeof err === "string") return err;
  if (err instanceof Error) {
    const raw = err.message ?? "";
    if (!raw) return null;
    if (raw.trim().startsWith("{")) {
      try {
        const parsed = JSON.parse(raw);
        if (parsed && typeof parsed.error === "string") {
          return parsed.error;
        }
      } catch {
        return raw;
      }
    }
    return raw;
  }
  return null;
}

function getNavigatorOnline(): boolean {
  if (typeof navigator === "undefined") return true;
  return navigator.onLine;
}

function createAnsiRenderer(): (input: string) => string {
  const colors: Record<number, string> = {
    30: "#1e1e1e",
    31: "#e06c75",
    32: "#98c379",
    33: "#e5c07b",
    34: "#61afef",
    35: "#c678dd",
    36: "#56b6c2",
    37: "#dcdfe4",
    90: "#7f848e",
    91: "#ff6b6b",
    92: "#b2f2bb",
    93: "#ffe066",
    94: "#74c0fc",
    95: "#e599f7",
    96: "#66d9e8",
    97: "#ffffff",
  };
  const bgColors: Record<number, string> = {
    40: "#1e1e1e",
    41: "#5c2a2a",
    42: "#2a4a2a",
    43: "#5c4a2a",
    44: "#2a3a5c",
    45: "#4a2a5c",
    46: "#2a5c5c",
    47: "#dcdfe4",
    100: "#3b3b3b",
    101: "#7a2e2e",
    102: "#2e6a2e",
    103: "#7a6a2e",
    104: "#2e4a7a",
    105: "#6a2e7a",
    106: "#2e7a7a",
    107: "#ffffff",
  };

  return (input: string) => {
    const esc = "\u001b[";
    // eslint-disable-next-line no-control-regex
    const regex = /\u001b\[[0-9;]*m/g;
    let lastIndex = 0;
    let fg: string | null = null;
    let bg: string | null = null;
    let out = "";

    const pushText = (text: string) => {
      const safe = escapeHtml(text);
      if (!fg && !bg) {
        out += safe;
        return;
      }
      const style = [
        fg ? `color:${fg}` : "",
        bg ? `background:${bg}` : "",
      ]
        .filter(Boolean)
        .join(";");
      out += `<span style="${style}">${safe}</span>`;
    };

    let match: RegExpExecArray | null;
    while ((match = regex.exec(input)) !== null) {
      const idx = match.index;
      if (idx > lastIndex) {
        pushText(input.slice(lastIndex, idx));
      }
      const seq = match[0].slice(esc.length, -1);
      const parts = seq.split(";").filter(Boolean).map(Number);
      if (parts.length === 0) {
        fg = null;
        bg = null;
      } else {
        for (const code of parts) {
          if (code === 0) {
            fg = null;
            bg = null;
          } else if (colors[code]) {
            fg = colors[code];
          } else if (bgColors[code]) {
            bg = bgColors[code];
          }
        }
      }
      lastIndex = regex.lastIndex;
    }
    if (lastIndex < input.length) {
      pushText(input.slice(lastIndex));
    }
    return out;
  };
}

function parseRunStatus(message: string): string | null {
  const trimmed = message.trim();
  if (!trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed) as { type?: string; status?: string };
    if (parsed?.type === "run_status" && typeof parsed.status === "string") {
      return parsed.status;
    }
  } catch {
    return null;
  }
  return null;
}

function isValidOutputPayload(
  payload: unknown
): payload is {
  event_id: number;
  agent_id: string;
  session_id: string;
  seq: string;
  ts: number;
  stream: OutputLine["stream"];
  message: string;
} {
  if (!payload || typeof payload !== "object") return false;
  const candidate = payload as {
    event_id?: unknown;
    agent_id?: unknown;
    session_id?: unknown;
    seq?: unknown;
    ts?: unknown;
    stream?: unknown;
    message?: unknown;
  };
  if (typeof candidate.event_id !== "number") return false;
  if (typeof candidate.agent_id !== "string" || !candidate.agent_id) return false;
  if (typeof candidate.session_id !== "string" || !candidate.session_id) return false;
  if (typeof candidate.seq !== "string" || !candidate.seq) return false;
  if (typeof candidate.ts !== "number") return false;
  if (typeof candidate.message !== "string") return false;
  if (
    candidate.stream !== "stdout" &&
    candidate.stream !== "stderr" &&
    candidate.stream !== "system" &&
    candidate.stream !== "acp"
  ) {
    return false;
  }
  return true;
}

function statusToAgentStatus(status: string): AgentRecord["status"] {
  if (status === "running") return "running";
  if (status === "idle") return "idle";
  if (status === "failed") return "failed";
  if (status === "completed" || status === "cancelled") return "stopped";
  return "stopped";
}

function isSamePermissionList(
  a: AcpPermissionRecord[],
  b: AcpPermissionRecord[]
): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    const left = a[i];
    const right = b[i];
    if (left.id !== right.id) return false;
    if (left.status !== right.status) return false;
    if (left.selected_option_id !== right.selected_option_id) return false;
    if ((left.responded_at ?? null) !== (right.responded_at ?? null)) {
      return false;
    }
  }
  return true;
}

export function filterPermissionsForAgent(
  items: AcpPermissionRecord[],
  agentId: string | null
): AcpPermissionRecord[] {
  if (!agentId) return [];
  return items.filter((item) => item.agent_id === agentId);
}
