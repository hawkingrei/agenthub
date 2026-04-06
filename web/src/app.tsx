import React, { Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  AgentRecord,
  AgentNodeRecord,
  AgentEvent,
  AuditRecord,
  AcpPermissionRecord,
  DeviceRecord,
  parseApiErrorMessage,
  SafePath,
  VapidInfo,
} from "./api";
import { buildAcpView, EMPTY_ACP_VIEW, type AcpView } from "./acp";
import {
  AGENT_NOT_RUNNING_ERROR,
  shouldIgnoreAgentWsError,
  sanitizeAgentError,
  isAgentActiveStatus,
  shouldShowUnexpectedExitNotice,
} from "./agent_ws";
import {
  clearAuthAndRedirect,
  isInvalidTokenMessage,
  resolvePostLoginRedirectTarget,
} from "./auth_redirect";
import {
  deriveConnectionBadge,
  getNavigatorOnline,
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
  isSseConnectionStale,
  isCursorNewer,
  shouldPollAgentEvents,
} from "./event_polling";
import { compareEventOrder } from "./seq_order";
import {
  buildLatestLiveSessionMap,
  normalizeSseOutputLines,
  resolveLiveSessionSwitch,
  routeLiveOutputBatch,
} from "./app_live_output";
import {
  buildGlobalPermissionPollAgentIds,
  chunkPermissionPollAgentIds,
  filterPermissionsForAgent,
  GLOBAL_PERMISSION_POLL_MAX_CONCURRENCY,
  mergePendingPermissionCountMap,
  parsePermissionPollAgentIds,
  resolveGlobalPermissionPollIntervalMs,
  schedulePermissionPollLoop,
} from "./app_permission_polling";
import {
  setupLayoutAnchorVarSync,
  setupRuntimeViewportVarSync,
} from "./app_viewport";
import {
  buildAcpCacheSlice,
  buildOutputCacheSlice,
  isSameOutputList,
  limitOutputCacheSessions,
  mergeOutputsPreserveOlderWithLimit,
  mergeOutputsWithLimit,
  mergeOutputsPreserveHistory,
  mergeOutputs,
  replaceAcpCacheSlice,
  OutputLine,
  selectCachedOutputs,
} from "./output_cache";
import { isNearBottom } from "./scroll";
import {
  DEFAULT_AGENT_PRESET_ID,
  formatAgentModelLabel,
  getAgentPreset,
  type AgentPresetId,
} from "./agent_presets";
import { validateAgentNodeDraft } from "./components/agent_node_validation";
import {
  buildAgentNodeSectionProps,
  buildCreateAgentModalProps,
  buildPermissionModalProps,
} from "./components/agents_route_modal_props";
import { AgentsRootPage } from "./components/agents_root_page";
import {
  buildAgentsPanelProps,
  buildAgentsWorkbenchProps,
  buildOutputHeaderProps,
} from "./components/agents_route_shell_props";
import { loadOutputCaches, saveOutputCaches } from "./storage/output_cache_storage";
import {
  DEFAULT_OUTPUT_CACHE_MAX_EVENTS,
  DEFAULT_OUTPUT_CACHE_MAX_SESSIONS,
} from "./storage/output_cache_budget";
import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "./storage/safe_storage";
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
  resolveDefaultWorktreeRootForTargetNode,
  normalizeWorkdirInput,
  resolveWorkdirForModeChange,
  resolveWorkdirForModalOpen,
  resolveWorkdirForTargetNodeChange,
} from "./worktree_defaults";
import { buildSseTargetAgentIds, encodeSseTargetAgentIds } from "./sse_targets";
import {
  AUTH_CARD_BASE_CLASS,
  AUTH_PAGE_CLASS,
  ROUTE_FALLBACK_SHELL_CLASS,
} from "./ui/tailwind_classes";
import {
  loadDeveloperModePreference,
  persistDeveloperModePreference,
} from "./ui/developer_mode";

const DEFAULT_WORKTREE_ROOT = "~/.agenthub/worktrees";
const PERMISSION_JUMP_MAX_ATTEMPTS = 24;
const SSE_STALE_RECONNECT_THRESHOLD_MS = 45_000;
const AGENTS_PANEL_WIDTH_STORAGE_KEY = "agenthub_agents_panel_width";
const AGENTS_PANEL_DEFAULT_WIDTH = 288;
const AGENTS_PANEL_MIN_WIDTH = 256;
const AGENTS_PANEL_MAX_WIDTH = 352;
const AGENTS_PANEL_MIN_RIGHT_WIDTH = 760;
const AGENTS_WORKSPACE_SPLITTER_WIDTH = 12;
const AGENTS_DESKTOP_BREAKPOINT_PX = 1024;
const AGENTS_PANEL_COMPACT_ROWS_THRESHOLD = 320;
const AGENT_STATUS_REFRESH_INTERVAL_MS = 10_000;
export const AGENT_EVENT_PAGE_SIZE = 80;
const LIVE_OUTPUT_RETENTION_LIMIT = 1200;
const LIVE_ACP_OUTPUT_RETENTION_LIMIT = 1200;

export {
  buildGlobalPermissionPollAgentIds,
  buildPendingPermissionCountMap,
  chunkPermissionPollAgentIds,
  filterPermissionsForAgent,
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

export function resolveDefaultActiveAgentId(agents: AgentRecord[]): string | null {
  return agents.find((agent) => isAgentActiveStatus(agent.status))?.id ?? null;
}

export function resolveActiveAcpView(
  activeAgent: string | null,
  acpOutputs: OutputLine[]
): AcpView {
  if (!activeAgent) {
    return EMPTY_ACP_VIEW;
  }
  return buildAcpView(acpOutputs);
}

export function buildPermissionPollAgentIds(agents: AgentRecord[]): string[] {
  return Array.from(
    new Set(
      agents
        .filter((agent) => isAgentActiveStatus(agent.status))
        .map((agent) => agent.id)
    )
  ).sort();
}

const LazyAdminPage = React.lazy(async () => {
  const module = (await import("./pages/admin_page")) as typeof import("./pages/admin_page");
  return { default: module.AdminPage };
});

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

function RouteFallback({ label }: { label: string }) {
  return <div className={ROUTE_FALLBACK_SHELL_CLASS}>{label}</div>;
}

function AuthGateCard({ title, message }: { title: string; message: string }) {
  return (
    <div className={AUTH_PAGE_CLASS}>
      <section className={AUTH_CARD_BASE_CLASS}>
        <h2 className="text-xl font-semibold tracking-tight text-slate-900">{title}</h2>
        <p className="mt-2 text-sm text-slate-600">{message}</p>
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

export function shouldRedirectTeamsToLogin(
  pathname: string,
  auth: AuthState | null,
  token: string | null
): boolean {
  return isTeamsRoute(pathname) && (!auth || !token);
}

export function canManageAgentNodes(auth: AuthState | null): boolean {
  return auth?.role === "root";
}

function compareAgentNodeRecords(a: AgentNodeRecord, b: AgentNodeRecord): number {
  if (a.is_main !== b.is_main) {
    return a.is_main ? -1 : 1;
  }
  if (a.created_at !== b.created_at) {
    return b.created_at - a.created_at;
  }
  return a.id.localeCompare(b.id);
}

export function upsertAgentNodeRecord(
  nodes: AgentNodeRecord[],
  node: AgentNodeRecord
): AgentNodeRecord[] {
  const existingIndex = nodes.findIndex((item) => item.id === node.id);
  if (existingIndex >= 0) {
    const next = [...nodes];
    next[existingIndex] = node;
    return next;
  }
  return [...nodes, node].sort(compareAgentNodeRecords);
}

function isSameStringList(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  for (let index = 0; index < a.length; index += 1) {
    if (a[index] !== b[index]) return false;
  }
  return true;
}

function isSameAgentRecord(a: AgentRecord, b: AgentRecord): boolean {
  return (
    a.id === b.id &&
    a.name === b.name &&
    a.workdir === b.workdir &&
    a.command === b.command &&
    isSameStringList(a.args, b.args) &&
    (a.target_node_id ?? null) === (b.target_node_id ?? null) &&
    a.worktree_mode === b.worktree_mode &&
    (a.worktree_repo ?? null) === (b.worktree_repo ?? null) &&
    (a.worktree_ref ?? null) === (b.worktree_ref ?? null) &&
    a.code_mode === b.code_mode &&
    (a.agent_loop_enabled ?? null) === (b.agent_loop_enabled ?? null) &&
    (a.agent_loop_idle_seconds ?? null) === (b.agent_loop_idle_seconds ?? null) &&
    (a.agent_loop_prompt ?? null) === (b.agent_loop_prompt ?? null) &&
    a.status === b.status &&
    a.created_at === b.created_at &&
    a.updated_at === b.updated_at
  );
}

export function isSameAgentRecordList(
  prev: AgentRecord[],
  next: AgentRecord[]
): boolean {
  if (prev.length !== next.length) return false;
  for (let index = 0; index < prev.length; index += 1) {
    if (!isSameAgentRecord(prev[index], next[index])) return false;
  }
  return true;
}

function isSameAgentNodeRecord(
  a: AgentNodeRecord,
  b: AgentNodeRecord
): boolean {
  return (
    a.id === b.id &&
    a.name === b.name &&
    (a.grpc_target ?? null) === (b.grpc_target ?? null) &&
    (a.tls_server_name ?? null) === (b.tls_server_name ?? null) &&
    (a.default_worktree_root ?? null) === (b.default_worktree_root ?? null) &&
    a.is_main === b.is_main &&
    a.created_at === b.created_at &&
    a.updated_at === b.updated_at
  );
}

export function isSameAgentNodeRecordList(
  prev: AgentNodeRecord[],
  next: AgentNodeRecord[]
): boolean {
  if (prev.length !== next.length) return false;
  for (let index = 0; index < prev.length; index += 1) {
    if (!isSameAgentNodeRecord(prev[index], next[index])) return false;
  }
  return true;
}

export function replaceAgentNodeRecord(
  nodes: AgentNodeRecord[],
  node: AgentNodeRecord
): AgentNodeRecord[] {
  return nodes.map((item) => (item.id === node.id ? node : item));
}

export function removeAgentNodeRecord(
  nodes: AgentNodeRecord[],
  nodeId: string
): AgentNodeRecord[] {
  return nodes.filter((item) => item.id !== nodeId);
}

function isTeamsRoute(pathname: string): boolean {
  return pathname === "/teams" || pathname === "/teams/" || pathname.startsWith("/teams/");
}

export function isAgentsWorkbenchRoute(pathname: string): boolean {
  return pathname === "/";
}

export function resolveTeamRoute(pathname: string): {
  mode: "selector" | "detail";
  teamId: string | null;
} | null {
  if (!isTeamsRoute(pathname)) {
    return null;
  }
  const suffix = pathname.slice("/teams".length);
  if (!suffix || suffix === "/") {
    return { mode: "selector", teamId: null };
  }
  const normalized = suffix.startsWith("/") ? suffix.slice(1) : suffix;
  const [rawTeamId] = normalized.split("/");
  if (!rawTeamId) {
    return { mode: "selector", teamId: null };
  }
  try {
    return {
      mode: "detail",
      teamId: decodeURIComponent(rawTeamId),
    };
  } catch {
    return {
      mode: "detail",
      teamId: rawTeamId,
    };
  }
}

export function resolvePostAuthRedirectTarget(
  pathname: string,
  search: string,
  auth: AuthState | null,
  token: string | null
): string | null {
  if (pathname !== "/") return null;
  if (!auth || !token) return null;
  return resolvePostLoginRedirectTarget(search);
}

type PendingPermissionJumpState = {
  toolCallId: string;
  sessionId: string | null;
  attempts: number;
};

export function decidePermissionJump(
  pending: PendingPermissionJumpState | null,
  acpTab: "conversation" | "plan" | "debug",
  activeSessionId: string | null,
  maxAttempts: number = PERMISSION_JUMP_MAX_ATTEMPTS
): PermissionJumpDecision {
  if (!pending) return "idle";
  if (acpTab !== "conversation") return "wait";
  if (pending.sessionId && activeSessionId !== pending.sessionId) return "wait";
  if (pending.attempts >= maxAttempts) return "clear";
  return "attempt";
}

export function resolveAgentsPanelMaxWidth(workspaceWidth: number): number {
  if (!Number.isFinite(workspaceWidth) || workspaceWidth <= 0) {
    return AGENTS_PANEL_MAX_WIDTH;
  }
  return Math.max(
    AGENTS_PANEL_MIN_WIDTH,
    Math.min(
      AGENTS_PANEL_MAX_WIDTH,
      Math.round(
        workspaceWidth -
          AGENTS_PANEL_MIN_RIGHT_WIDTH -
          AGENTS_WORKSPACE_SPLITTER_WIDTH
      )
    )
  );
}

export function clampAgentsPanelWidth(
  value: number,
  maxWidth = AGENTS_PANEL_MAX_WIDTH
): number {
  if (!Number.isFinite(value)) {
    return AGENTS_PANEL_DEFAULT_WIDTH;
  }
  const effectiveMax = Math.max(
    AGENTS_PANEL_MIN_WIDTH,
    Math.min(AGENTS_PANEL_MAX_WIDTH, maxWidth)
  );
  return Math.max(
    AGENTS_PANEL_MIN_WIDTH,
    Math.min(effectiveMax, Math.round(value))
  );
}

export function loadAgentsPanelWidthPreference(
  raw = getLocalStorageItemSafe(AGENTS_PANEL_WIDTH_STORAGE_KEY)
): number {
  const parsed = raw ? Number.parseInt(raw, 10) : Number.NaN;
  return clampAgentsPanelWidth(parsed);
}

export function persistAgentsPanelWidthPreference(width: number): void {
  setLocalStorageItemSafe(
    AGENTS_PANEL_WIDTH_STORAGE_KEY,
    String(clampAgentsPanelWidth(width))
  );
}

export function resolveOutputHistoryKey(
  agentId: string,
  sessionId: string | null,
  agentSessions: Record<string, string>
): string {
  if (sessionId) return `${agentId}:${sessionId}`;
  const latestSessionId = agentSessions[agentId] ?? "latest";
  return `${agentId}:latest:${latestSessionId}`;
}

export function resolveSessionScopedEvents(
  events: AgentEvent[],
  requestedSessionId: string | null
): {
  latestSessionId: string | null;
  resolvedSessionId: string | null;
  scopedEvents: AgentEvent[];
} {
  const latestSessionId = !requestedSessionId
    ? events.reduce<AgentEvent | null>((latest, current) => {
        if (!current.session_id) return latest;
        if (!latest) return current;
        return compareEventOrder(current, latest) > 0 ? current : latest;
      }, null)?.session_id ?? null
    : null;
  const resolvedSessionId = requestedSessionId ?? latestSessionId;
  const scopedEvents = resolvedSessionId
    ? events.filter((evt) => evt.session_id === resolvedSessionId)
    : events;
  return {
    latestSessionId,
    resolvedSessionId,
    scopedEvents,
  };
}

export function App() {
  const eventLimit = AGENT_EVENT_PAGE_SIZE;
  const maxCachedEvents = DEFAULT_OUTPUT_CACHE_MAX_EVENTS;
  const maxCachedSessions = DEFAULT_OUTPUT_CACHE_MAX_SESSIONS;
  const [routeLocation, setRouteLocation] = useState(() => ({
    pathname: location.pathname,
    search: location.search,
  }));
  const isAdminRoute = routeLocation.pathname.startsWith("/admin");
  const isAgentsRoute = isAgentsWorkbenchRoute(routeLocation.pathname);
  const navigateWorkbenchRoute = useCallback(
    (pathname: string) => {
      if (routeLocation.pathname === pathname) {
        return;
      }
      window.history.pushState({}, "", pathname);
      window.dispatchEvent(new PopStateEvent("popstate"));
    },
    [routeLocation.pathname]
  );
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
  const [agentNodes, setAgentNodes] = useState<AgentNodeRecord[]>([]);
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
  const [targetNodeId, setTargetNodeId] = useState("main");
  const [nodeIdInput, setNodeIdInput] = useState("");
  const [nodeNameInput, setNodeNameInput] = useState("");
  const [nodeGrpcTargetInput, setNodeGrpcTargetInput] = useState("");
  const [nodeTlsServerNameInput, setNodeTlsServerNameInput] = useState("");
  const [nodeDefaultWorktreeRootInput, setNodeDefaultWorktreeRootInput] = useState("");
  const [createAgentNodeBusy, setCreateAgentNodeBusy] = useState(false);
  const [updatingAgentNodeIds, setUpdatingAgentNodeIds] = useState<
    Record<string, boolean>
  >({});
  const [deletingAgentNodeIds, setDeletingAgentNodeIds] = useState<
    Record<string, boolean>
  >({});
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>(
    []
  );
  const [pendingPermissionCounts, setPendingPermissionCounts] = useState<
    Record<string, number>
  >({});
  const [startingAgentIds, setStartingAgentIds] = useState<Record<string, boolean>>({});
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const ansi = useMemo(() => createAnsiRenderer(), []);
  const [input, setInput] = useState("");
  const [inputHistory, setInputHistory] = useState<string[]>(() =>
    parseInputHistory(getLocalStorageItemSafe(INPUT_HISTORY_STORAGE_KEY))
  );
  const [inputHistoryCursor, setInputHistoryCursor] = useState(-1);
  const inputHistoryDraftRef = useRef("");
  const sseRef = useRef<EventSource | null>(null);
  const lastSseActivityAtRef = useRef<number>(Date.now());
  const terminalRef = useRef<HTMLDivElement | null>(null);
  const terminalStickToBottomRef = useRef(true);
  const [terminalShowJump, setTerminalShowJump] = useState(false);
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
  const [agentsPanelWidth, setAgentsPanelWidth] = useState(() =>
    loadAgentsPanelWidthPreference()
  );
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [passkeyEnabled, setPasskeyEnabled] = useState<boolean | null>(null);
  const [authBusy, setAuthBusy] = useState<"login" | "register" | null>(null);
  const [developerMode, setDeveloperMode] = useState<boolean>(() =>
    loadDeveloperModePreference()
  );
  const [acpTab, setAcpTab] = useState<"conversation" | "plan" | "debug">(
    "conversation"
  );
  const handleDeveloperModeChange = useCallback((next: boolean) => {
    setDeveloperMode(next);
    persistDeveloperModePreference(next);
  }, []);
  const handleAcpTabSelect = useCallback(
    (next: "conversation" | "plan" | "debug") => {
      if (!developerMode && next === "debug") {
        setAcpTab("conversation");
        return;
      }
      setAcpTab(next);
    },
    [developerMode]
  );
  const [createAgentBusy, setCreateAgentBusy] = useState(false);
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<
    AcpPermissionRecord[]
  >([]);
  const appRootRef = useRef<HTMLDivElement | null>(null);
  const appHeaderRef = useRef<HTMLElement | null>(null);
  const workspaceRef = useRef<HTMLElement | null>(null);
  const agentsPanelWidthRef = useRef(agentsPanelWidth);
  const agentsResizeCleanupRef = useRef<(() => void) | null>(null);
  const [, setThinkingTick] = useState(0);
  const createAgentBusyRef = useRef(false);
  const authBusyRef = useRef<"login" | "register" | null>(null);
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
  const activeAgentPrevStatusRef = useRef<Record<string, string | null>>({});
  const requestSseReconnectRef = useRef<(() => void) | null>(null);
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
    agentsPanelWidthRef.current = agentsPanelWidth;
  }, [agentsPanelWidth]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const syncAgentsPanelWidth = () => {
      if (window.innerWidth <= AGENTS_DESKTOP_BREAKPOINT_PX) {
        return;
      }
      const workspaceWidth = workspaceRef.current?.getBoundingClientRect().width ?? 0;
      if (workspaceWidth <= 0) {
        return;
      }
      const nextMaxWidth = resolveAgentsPanelMaxWidth(workspaceWidth);
      setAgentsPanelWidth((current) => clampAgentsPanelWidth(current, nextMaxWidth));
    };
    syncAgentsPanelWidth();
    window.addEventListener("resize", syncAgentsPanelWidth);
    return () => {
      window.removeEventListener("resize", syncAgentsPanelWidth);
    };
  }, [agentsCollapsed]);

  useEffect(() => {
    return () => {
      agentsResizeCleanupRef.current?.();
    };
  }, []);

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
    setTerminalShowJump(!stick);
  }, []);
  const jumpToTerminalBottom = useCallback(() => {
    const el = terminalRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
    terminalStickToBottomRef.current = true;
    setTerminalShowJump(false);
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
        const nextCache = limitOutputCacheSessions(
          { ...outputCacheRef.current, [key]: nextSlice },
          maxCachedSessions
        );
        outputCacheRef.current = nextCache;
        setOutputCache(nextCache);
      }
      return nextSlice;
    },
    [maxCachedEvents, maxCachedSessions]
  );
  const updateAcpOutputCacheEntry = useCallback(
    (key: string, ordered: OutputLine[]) => {
      const existing = acpOutputCacheRef.current[key] ?? [];
      const nextSlice = buildAcpCacheSlice(existing, ordered, maxCachedEvents);
      if (!isSameOutputList(existing, nextSlice)) {
        const nextCache = limitOutputCacheSessions(
          { ...acpOutputCacheRef.current, [key]: nextSlice },
          maxCachedSessions
        );
        acpOutputCacheRef.current = nextCache;
        setAcpOutputCache(nextCache);
      }
      return nextSlice;
    },
    [maxCachedEvents, maxCachedSessions]
  );
  const replaceAcpOutputCacheEntry = useCallback(
    (key: string, ordered: OutputLine[]) => {
      const existing = acpOutputCacheRef.current[key] ?? [];
      const nextSlice = replaceAcpCacheSlice(ordered, maxCachedEvents);
      if (!isSameOutputList(existing, nextSlice)) {
        const nextCache = limitOutputCacheSessions(
          { ...acpOutputCacheRef.current, [key]: nextSlice },
          maxCachedSessions
        );
        acpOutputCacheRef.current = nextCache;
        setAcpOutputCache(nextCache);
      }
      return nextSlice;
    },
    [maxCachedEvents, maxCachedSessions]
  );
  const consumeLiveOutputBatch = useCallback(
    (lines: OutputLine[]) => {
      if (lines.length === 0) return;
      const currentActive = activeAgentRef.current;
      const currentSessionId = activeSessionIdRef.current;
      const { activeLines, activeAcpLines } = routeLiveOutputBatch({
        cursorRef: lastEventCursorRef,
        lines,
        activeAgent: currentActive,
        activeSessionId: currentSessionId,
        updateAgents: setAgents,
        onOutputGroup: updateOutputCacheEntry,
        onAcpGroup: updateAcpOutputCacheEntry,
      });
      const latestSessions = buildLatestLiveSessionMap(lines);
      if (Object.keys(latestSessions).length > 0) {
        setAgentSessions((prev) => {
          let next = prev;
          for (const [agentId, sessionId] of Object.entries(latestSessions)) {
            if (prev[agentId] === sessionId || next[agentId] === sessionId) {
              continue;
            }
            if (next === prev) {
              next = { ...prev };
            }
            next[agentId] = sessionId;
          }
          return next;
        });
      }
      const liveSessionSwitch = resolveLiveSessionSwitch(
        lines,
        currentActive,
        currentSessionId
      );
      if (
        liveSessionSwitch &&
        liveSessionSwitch !== currentSessionId &&
        isAgentActiveStatus(activeAgentStatusRef.current)
      ) {
        setActiveSessionId(liveSessionSwitch);
      }

      if (activeLines.length > 0) {
        setOutputs((prev) =>
          mergeOutputsWithLimit(prev, activeLines, LIVE_OUTPUT_RETENTION_LIMIT)
        );
      }
      if (activeAcpLines.length > 0) {
        setAcpOutputs((prev) =>
          mergeOutputsWithLimit(prev, activeAcpLines, LIVE_ACP_OUTPUT_RETENTION_LIMIT)
        );
      }
    },
    [updateAcpOutputCacheEntry, updateOutputCacheEntry]
  );
  const acpView = useMemo(
    () => resolveActiveAcpView(activeAgent, acpOutputs),
    [activeAgent, acpOutputs]
  );
  const terminalOutputs = useMemo(
    () => outputs.filter((line) => line.stream !== "acp"),
    [outputs]
  );
  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );
  const selectedTargetNodeDefaultWorktreeRoot = useMemo(
    () =>
      resolveDefaultWorktreeRootForTargetNode(
        targetNodeId,
        agentNodes,
        defaultWorktreeRoot,
        DEFAULT_WORKTREE_ROOT
      ),
    [agentNodes, defaultWorktreeRoot, targetNodeId]
  );
  const selectedTargetNodeDefaultWorktreeRootRef = useRef(
    selectedTargetNodeDefaultWorktreeRoot
  );
  const agentNodesRef = useRef(agentNodes);
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
  const hasPendingPermissions = useMemo(() => {
    return Object.values(pendingPermissionCounts).some((count) => count > 0);
  }, [pendingPermissionCounts]);
  const activeAgentStatus = activeAgentRecord?.status ?? null;
  const isAgentActive = isAgentActiveStatus(activeAgentStatus);
  const streamAgentIds = useMemo(() => buildSseTargetAgentIds(agents), [agents]);
  const streamAgentIdsQuery = useMemo(
    () => encodeSseTargetAgentIds(streamAgentIds),
    [streamAgentIds]
  );
  const permissionPollAgentIds = useMemo(
    () => buildPermissionPollAgentIds(agents),
    [agents]
  );
  const permissionPollAgentIdsKey = useMemo(
    () => permissionPollAgentIds.join(","),
    [permissionPollAgentIds]
  );
  const hasSseTarget = streamAgentIds.length > 0;
  const connectionBadge = useMemo(
    () => deriveConnectionBadge(networkOnline, hasSseTarget, sseState),
    [networkOnline, hasSseTarget, sseState]
  );
  const thinkingStartTs = acpView.thinkingStartTs;
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
  const isConversationLoading =
    Boolean(activeAgent) &&
    acpTab === "conversation" &&
    (activeAgentRecord?.code_mode ?? true) &&
    !acpView.hasAcp;

  const token = auth?.token ?? null;
  const postAuthRedirectTarget = resolvePostAuthRedirectTarget(
    routeLocation.pathname,
    routeLocation.search,
    auth,
    token
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
  useEffect(() => {
    activeAgentRef.current = activeAgent;
  }, [activeAgent]);
  useEffect(() => {
    activeSessionIdRef.current = activeSessionId;
  }, [activeSessionId]);
  useEffect(() => {
    selectedTargetNodeDefaultWorktreeRootRef.current =
      selectedTargetNodeDefaultWorktreeRoot;
  }, [selectedTargetNodeDefaultWorktreeRoot]);
  useEffect(() => {
    agentNodesRef.current = agentNodes;
  }, [agentNodes]);
  useEffect(() => {
    activeAgentStatusRef.current = activeAgentStatus;
  }, [activeAgentStatus]);
  useEffect(() => {
    if (!activeAgent) return;
    const prev = activeAgentPrevStatusRef.current[activeAgent] ?? null;
    const next = activeAgentStatus;
    activeAgentPrevStatusRef.current[activeAgent] = next;
    if (!shouldShowUnexpectedExitNotice(prev, next)) return;
    setError(
      `Agent process exited unexpectedly (status: ${next}). Please restart the agent.`
    );
  }, [activeAgent, activeAgentStatus]);
  useEffect(() => {
    setAcpPermissions([]);
    setAcpPermissionHistory([]);
  }, [activeAgent]);

  useEffect(() => {
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, [activeAgent, activeSessionId]);

  const refreshAgents = useCallback(
    async (opts?: { silent?: boolean }): Promise<AgentRecord[] | null> => {
      if (!token) return null;
      const silent = opts?.silent === true;
      try {
        const items = await api.listAgents(token);
        setAgents((prev) => (isSameAgentRecordList(prev, items) ? prev : items));
        return items;
      } catch (err) {
        if (!silent) {
          setError(formatWorktreeError(err) ?? parseApiErrorMessage(err) ?? String(err));
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
      } catch (err) {
        if (!silent) {
          setError(parseApiErrorMessage(err) ?? String(err));
        }
        return null;
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
    const timer = window.setInterval(() => {
      void refreshAgents({ silent: true });
    }, AGENT_STATUS_REFRESH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [isAgentsRoute, token, refreshAgents]);

  useEffect(() => {
    if (activeAgent || agents.length === 0) return;
    const nextAgentId = resolveDefaultActiveAgentId(agents);
    if (nextAgentId) {
      setActiveAgent(nextAgentId);
      setActiveSessionId(agentSessions[nextAgentId] ?? null);
    }
  }, [agents, activeAgent, agentSessions]);

  useEffect(() => {
    api
      .authStatus()
      .then((res) => {
        setRootInitialized(res.root_initialized);
        setPasskeyEnabled(res.passkey_enabled);
      })
      .catch(() => {});
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
      const { latestSessionId, resolvedSessionId, scopedEvents } =
        resolveSessionScopedEvents(events, sessionId ?? null);
      if (latestSessionId) {
        setAgentSessions((prev) => ({ ...prev, [id]: latestSessionId }));
        if (activeAgentRef.current === id && !activeSessionIdRef.current) {
          setActiveSessionId(latestSessionId);
        }
      }
      const ordered = [...scopedEvents].sort((a, b) => compareEventOrder(a, b));
      const resolvedKey = `${id}:${resolvedSessionId ?? "latest"}`;
      const nextSlice = updateOutputCacheEntry(key, ordered);
      if (resolvedKey !== key) {
        updateOutputCacheEntry(resolvedKey, ordered);
      }
      if (sessionId) {
        updateAcpOutputCacheEntry(key, ordered);
      } else {
        replaceAcpOutputCacheEntry(key, ordered);
      }
      if (resolvedKey !== key) {
        updateAcpOutputCacheEntry(resolvedKey, ordered);
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
        const apply = (metaKey: string, currentState: typeof prev) => {
          const current = currentState[metaKey];
          if (
            current &&
            current.oldestId === nextMeta.oldestId &&
            current.hasMore === nextMeta.hasMore &&
            current.loading === nextMeta.loading &&
            current.loaded === nextMeta.loaded
          ) {
            return currentState;
          }
          return { ...currentState, [metaKey]: nextMeta };
        };
        let nextState = apply(key, prev);
        if (resolvedKey !== key) {
          nextState = apply(resolvedKey, nextState);
        }
        return nextState;
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
    replaceAcpOutputCacheEntry,
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
      setOutputs((prev) =>
        mergeOutputsPreserveOlderWithLimit(prev, ordered, LIVE_OUTPUT_RETENTION_LIMIT)
      );
      setAcpOutputs((prev) =>
        mergeOutputsPreserveOlderWithLimit(prev, acpOrdered, LIVE_ACP_OUTPUT_RETENTION_LIMIT)
      );
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
    const viewKey = resolveOutputHistoryKey(
      activeAgent,
      activeSessionId,
      agentSessions
    );
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
      outputsKeyRef.current = viewKey;
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
    const shouldPreserveHistory = previousKey === viewKey;
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
    outputsKeyRef.current = viewKey;
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
    agentSessions,
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
          selectedTargetNodeDefaultWorktreeRoot,
          DEFAULT_WORKTREE_ROOT
        )
      );
    },
    [selectedTargetNodeDefaultWorktreeRoot]
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
  }, [agentNodes, defaultWorktreeRoot, refreshAgentNodes, worktreeMode]);

  const closeCreateAgentModal = useCallback(() => {
    setShowCreateAgent(false);
  }, []);

  useEffect(() => {
    if (!token || auth?.role !== "root" || !isAdminRoute) {
      setSafePaths([]);
      setDevices([]);
      setAudits([]);
      setVapidInfo(null);
      return;
    }
    api.listSafePaths(token).then(setSafePaths).catch(() => {});
    api.listDevices(token).then(setDevices).catch(() => {});
    api.listAudits(token).then(setAudits).catch(() => {});
    api.getVapidInfo(token).then(setVapidInfo).catch(() => {});
  }, [token, auth?.role, isAdminRoute]);

  useEffect(() => {
    setError((prev) => sanitizeAgentError(prev, activeAgentStatus));
  }, [activeAgentStatus]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent) return;
    loadAgentEvents(activeAgent, activeSessionId);
  }, [isAgentsRoute, token, activeAgent, activeSessionId, loadAgentEvents]);

  useEffect(() => {
    if (!isAgentsRoute || !token) {
      setSseState("idle");
      requestSseReconnectRef.current = null;
      return;
    }
    const streamTarget = streamAgentIdsQuery;
    if (!hasSseTarget || streamTarget.length === 0) {
      setSseState("idle");
      requestSseReconnectRef.current = null;
      return;
    }
    let cancelled = false;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;
    const clearReconnectTimer = () => {
      if (reconnectTimer) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
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
    requestSseReconnectRef.current = scheduleReconnect;
    function openSource() {
      if (cancelled) return;
      setSseState("connecting");
      const source = new EventSource(
        `${location.origin}/sse/agents?ids=${encodeURIComponent(
          streamTarget
        )}&token=${encodeURIComponent(token)}`
      );
      sseRef.current = source;
      source.onopen = () => {
        lastSseActivityAtRef.current = Date.now();
        reconnectAttempt = 0;
        setNetworkOnline(true);
        setSseState("connected");
        const pollState = eventPollRef.current;
        if (pollState.timer) {
          window.clearTimeout(pollState.timer);
          pollState.timer = null;
        }
      };
      source.onmessage = (event) => {
        lastSseActivityAtRef.current = Date.now();
        if (event.data === "heartbeat") return;
        try {
          const parsed = JSON.parse(event.data);
          const liveLines = normalizeSseOutputLines(parsed);
          if (liveLines.length > 0) {
            consumeLiveOutputBatch(liveLines);
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
        source.close();
        sseRef.current = null;
        schedulePollRef.current?.(
          getAdaptivePollInterval(eventPollRef.current.idleCount)
        );
        const nextError = online ? UPSTREAM_HTML_MESSAGE : OFFLINE_MESSAGE;
        void (async () => {
          const latestAgents = await refreshAgents({ silent: true });
          if (cancelled) return;
          if (latestAgents) {
            const nextTargets = buildSseTargetAgentIds(latestAgents);
            if (nextTargets.length === 0) {
              setSseState("idle");
              setError((prev) =>
                prev === OFFLINE_MESSAGE || prev === UPSTREAM_HTML_MESSAGE
                  ? null
                  : prev
              );
              return;
            }
          }
          setError(nextError);
          scheduleReconnect();
        })();
      };
    }
    openSource();
    return () => {
      cancelled = true;
      clearReconnectTimer();
      requestSseReconnectRef.current = null;
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
      setSseState("idle");
    };
  }, [
    isAgentsRoute,
    token,
    hasSseTarget,
    streamAgentIdsQuery,
    consumeLiveOutputBatch,
    refreshAgents,
  ]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent) {
      const pollState = eventPollRef.current;
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
        pollState.timer = null;
      }
      pollState.idleCount = 0;
      pollState.boostUntil = null;
      schedulePollRef.current = null;
      return;
    }

    let cancelled = false;
    const pollState = eventPollRef.current;
    const pollActiveAgent = async (): Promise<boolean> => {
      const currentActive = activeAgentRef.current;
      if (!currentActive) return false;
      return (
        (await loadAgentEvents(currentActive, activeSessionIdRef.current)) === true
      );
    };
    const schedulePoll = (delay: number) => {
      if (cancelled) return;
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
      }
      const now = Date.now();
      const boostUntil = pollState.boostUntil;
      const boostActive = boostUntil != null && boostUntil > now;
      const isSseOpen = sseRef.current?.readyState === EventSource.OPEN;
      const sseStale = isSseConnectionStale(
        Boolean(isSseOpen),
        lastSseActivityAtRef.current,
        now,
        SSE_STALE_RECONNECT_THRESHOLD_MS
      );
      if (isSseOpen && sseStale) {
        const current = sseRef.current;
        if (current) {
          current.close();
          if (sseRef.current === current) {
            sseRef.current = null;
          }
        }
        setSseState("reconnecting");
        requestSseReconnectRef.current?.();
      }
      if (isSseOpen && !boostActive && !sseStale) {
        pollState.timer = null;
        return;
      }
      const nextDelay = boostActive ? 1000 : delay;
      pollState.timer = window.setTimeout(async () => {
        if (cancelled) return;
        const current = sseRef.current;
        const isOpen =
          current != null && current.readyState === EventSource.OPEN;
        const callbackNow = Date.now();
        const callbackSseStale = isSseConnectionStale(
          isOpen,
          lastSseActivityAtRef.current,
          callbackNow,
          SSE_STALE_RECONNECT_THRESHOLD_MS
        );
        if (isOpen && callbackSseStale) {
          current?.close();
          if (sseRef.current === current) {
            sseRef.current = null;
          }
          setSseState("reconnecting");
          requestSseReconnectRef.current?.();
        }
        const shouldPoll = shouldPollAgentEvents(
          isOpen,
          pollState.boostUntil,
          callbackNow,
          callbackSseStale
        );
        if (!shouldPoll && pollState.boostUntil != null) {
          pollState.boostUntil = null;
        }
        let hasNew = false;
        if (shouldPoll) {
          hasNew = (await pollActiveAgent()) === true;
        } else {
          pollState.idleCount = 0;
        }
        if (hasNew) {
          pollState.idleCount = 0;
        } else if (shouldPoll) {
          pollState.idleCount += 1;
        }
        if (!cancelled) {
          schedulePoll(getAdaptivePollInterval(pollState.idleCount));
        }
      }, nextDelay);
    };

    schedulePollRef.current = schedulePoll;
    schedulePoll(getAdaptivePollInterval(pollState.idleCount));
    return () => {
      cancelled = true;
      if (pollState.timer) {
        window.clearTimeout(pollState.timer);
        pollState.timer = null;
      }
      pollState.idleCount = 0;
      pollState.boostUntil = null;
      schedulePollRef.current = null;
    };
  }, [isAgentsRoute, token, activeAgent, activeSessionId, loadAgentEvents]);

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

  useEffect(() => {
    if (!isAgentsRoute || !token || !permissionPollAgentIdsKey) {
      setPendingPermissionCounts({});
      return;
    }
    let cancelled = false;
    const allAgentIds = parsePermissionPollAgentIds(
      permissionPollAgentIdsKey
    );
    const requestedAgentIds = buildGlobalPermissionPollAgentIds(
      allAgentIds,
      activeAgent
    );
    const requestedChunks = chunkPermissionPollAgentIds(
      requestedAgentIds,
      GLOBAL_PERMISSION_POLL_MAX_CONCURRENCY
    );
    const pollIntervalMs = resolveGlobalPermissionPollIntervalMs(agentsCollapsed);
    const load = async () => {
      const entries: Array<readonly [string, number | null]> = [];
      for (const chunk of requestedChunks) {
        const batch = await Promise.all(
          chunk.map(async (agentId) => {
            try {
              const items = await api.listAcpPermissions(token, agentId, "pending");
              return [agentId, items.length] as const;
            } catch {
              return [agentId, null] as const;
            }
          })
        );
        entries.push(...batch);
        if (cancelled) return;
      }
      if (cancelled) return;
      setPendingPermissionCounts((prev) =>
        (() => {
          const nextCounts = mergePendingPermissionCountMap(
            prev,
            allAgentIds,
            entries
          );
          return isSamePendingPermissionCountMap(prev, nextCounts)
            ? prev
            : nextCounts;
        })()
      );
    };
    load();
    const timer = window.setInterval(load, pollIntervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [isAgentsRoute, token, permissionPollAgentIdsKey, activeAgent, agentsCollapsed]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent) {
      setAcpPermissions([]);
      return;
    }
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
          setPendingPermissionCounts((prev) => {
            const nextCount = items.length;
            if (nextCount <= 0) {
              if (!(requestedAgentId in prev)) return prev;
              const next = { ...prev };
              delete next[requestedAgentId];
              return next;
            }
            if (prev[requestedAgentId] === nextCount) return prev;
            return { ...prev, [requestedAgentId]: nextCount };
          });
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
  }, [isAgentsRoute, token, activeAgent, activeSessionId]);

  useEffect(() => {
    if (!thinkingStartTs) return;
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [thinkingStartTs]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent || !developerMode || acpTab !== "debug") {
      setAcpPermissionHistory([]);
      return;
    }
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
  }, [isAgentsRoute, token, activeAgent, activeSessionId, developerMode, acpTab]);

  const onRegister = async (role?: string) => {
    if (authBusyRef.current) return;
    authBusyRef.current = "register";
    setAuthBusy("register");
    setError(null);
    try {
      const start = await api.registerStart(
        username,
        displayName,
        role,
        role === "root" ? password : undefined
      );

      let next: AuthState;
      if (start.token && start.user_id) {
        next = {
          token: start.token,
          userId: start.user_id,
          username,
          role: start.role ?? role ?? "device",
        };
      } else {
        if (!start.challenge_id || !start.options) {
          throw new Error("invalid registration response: missing challenge");
        }
        const options = publicKeyCredentialCreationOptionsFromJson(start.options);
        const cred = await navigator.credentials.create({ publicKey: options });
        if (!cred) throw new Error("registration cancelled");
        const payload = registerCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.registerFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      }

      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(next.token);
    } catch (err) {
      setError(formatWorktreeError(err) ?? parseApiErrorMessage(err) ?? String(err));
    } finally {
      authBusyRef.current = null;
      setAuthBusy(null);
    }
  };

  const onLogin = async () => {
    if (authBusyRef.current) return;
    authBusyRef.current = "login";
    setAuthBusy("login");
    setError(null);
    try {
      const start = await api.loginStart(username, password);

      let next: AuthState;
      if (start.token && start.user_id) {
        next = {
          token: start.token,
          userId: start.user_id,
          username,
          role: start.role ?? "unknown",
        };
      } else if (start.registration_options) {
        if (!start.challenge_id) {
          throw new Error("invalid login response: missing challenge for registration");
        }
        const options = publicKeyCredentialCreationOptionsFromJson(start.registration_options);
        const cred = await navigator.credentials.create({ publicKey: options });
        if (!cred) throw new Error("registration cancelled");
        const payload = registerCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.loginRegisterFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      } else {
        if (!start.challenge_id || !start.options) {
          throw new Error("invalid login response: missing challenge");
        }
        const options = publicKeyCredentialRequestOptionsFromJson(start.options);
        const cred = await navigator.credentials.get({ publicKey: options });
        if (!cred) throw new Error("login cancelled");
        const payload = loginCredentialToJson(cred as PublicKeyCredential);
        const finish = await api.loginFinish(start.challenge_id, payload);
        next = {
          token: finish.token,
          userId: finish.user_id,
          username,
          role: finish.role,
        };
      }

      setLocalStorageItemSafe("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(next.token);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    } finally {
      authBusyRef.current = null;
      setAuthBusy(null);
    }
  };

  const onLogout = () => {
    removeLocalStorageItemSafe("agenthub_auth");
    setAuth(null);
    setAgents([]);
    setAgentNodes([]);
    setActiveAgent(null);
    setOutputs([]);
    setAcpOutputs([]);
    setSafePaths([]);
    setDevices([]);
    setAcpPermissions([]);
    setPendingPermissionCounts({});
    setVapidInfo(null);
    setWorktreeError(null);
    setDefaultWorktreeRoot(DEFAULT_WORKTREE_ROOT);
    setAgentWorkdir("");
    setTargetNodeId("main");
    setNodeIdInput("");
    setNodeNameInput("");
    setNodeGrpcTargetInput("");
    setNodeTlsServerNameInput("");
    setNodeDefaultWorktreeRootInput("");
    setCreateAgentNodeBusy(false);
    setUpdatingAgentNodeIds({});
    setDeletingAgentNodeIds({});
  };

  const applyTargetNodeSelection = useCallback(
    (nextTargetNodeId: string, nextNodes: AgentNodeRecord[] = agentNodes) => {
      const resolvedTargetNodeId = nextTargetNodeId.trim() || "main";
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
    if (targetNodeId === "main") {
      return;
    }
    if (agentNodes.some((node) => node.id === targetNodeId)) {
      return;
    }
    applyTargetNodeSelection("main");
  }, [agentNodes, applyTargetNodeSelection, targetNodeId]);

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
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
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
      } catch (err) {
        setError(parseApiErrorMessage(err) ?? String(err));
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
      } catch (err) {
        setError(parseApiErrorMessage(err) ?? String(err));
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
    if (!token) return;
    if (createAgentBusyRef.current) return;
    createAgentBusyRef.current = true;
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
        selectedTargetNodeDefaultWorktreeRoot
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
        workdir: workdirPayload,
        command,
        args,
        target_node_id: normalizedTargetNodeId,
        worktree_mode: worktreeMode,
        worktree_repo: worktreeRepo.trim() || null,
        worktree_ref: worktreeRef.trim() || null,
        code_mode: codeMode,
      });
      setAgents((prev) => [agent, ...prev]);
      setActiveAgent(agent.id);
      try {
        const res = await api.startAgent(token, agent.id);
        setActiveSessionId(res.session_id);
        setAgentSessions((prev) => ({ ...prev, [agent.id]: res.session_id }));
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
      applyTargetNodeSelection("main");
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
  }, [
    token,
    targetNodeId,
    agentName,
    agentWorkdir,
    selectedTargetNodeDefaultWorktreeRoot,
    worktreeMode,
    agentPresetId,
    worktreeRepo,
    worktreeRef,
    codeMode,
    refreshAgents,
    applyTargetNodeSelection,
  ]);

  const onStartAgent = useCallback(async (id: string) => {
    if (!token) return;
    setError(null);
    setWorktreeError(null);
    setStartingAgentIds((prev) => ({ ...prev, [id]: true }));
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
      setActiveSessionId(null);
      await refreshAgents();
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  }, [token]);

  const onAcpSetMode = useCallback(async (requestedModeId: string) => {
    if (!token || !activeAgent) return;
    const modeId = requestedModeId.trim();
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
  }, [token, activeAgent]);

  const onAcpSetModel = useCallback(async (requestedModelId: string) => {
    if (!token || !activeAgent) return;
    const modelId = requestedModelId.trim();
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
  }, [token, activeAgent]);

  const onAcpSetConfig = useCallback(async () => {
    if (!token || !activeAgent) return;
    const trimmedId = acpConfigId.trim();
    const trimmedValue = acpConfigValue.trim();
    if (!trimmedId || !trimmedValue) {
      setError("config id and value are required");
      return;
    }
    setError(null);
    try {
      await api.setAcpConfig(token, activeAgent, trimmedId, trimmedValue);
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

  const sendAcpInput = useCallback(async (
    rawText: string,
    options?: {
      recordHistory?: boolean;
      clearComposer?: boolean;
    }
  ) => {
    const text = rawText.trim();
    if (!text) return;
    if (!token || !activeAgent) return;
    eventPollRef.current.boostUntil = Date.now() + 10_000;
    schedulePollRef.current?.(1000);
    let messageId: string | null = null;
    if (activeSessionId) {
      messageId =
        typeof crypto !== "undefined" && "randomUUID" in crypto
          ? crypto.randomUUID()
          : `local-${Date.now()}`;
    }
    const sendInputForSession = (sessionId: string | null) =>
      api.sendInput(
        token,
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
    } catch (err) {
      const msg = parseApiErrorMessage(err) ?? String(err || "websocket not connected");
      const mismatch = parseSendInputSessionMismatch(msg);
      if (mismatch) {
        const runningSessionId = mismatch.running;
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
        } catch (retryErr) {
          const retryMsg =
            parseApiErrorMessage(retryErr) ??
            String(retryErr || "websocket not connected");
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
    token,
    activeAgent,
    activeSessionId,
    loadAgentEvents,
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

  const handleAgentsSplitterPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      if (typeof window === "undefined") return;
      if (window.innerWidth <= AGENTS_DESKTOP_BREAKPOINT_PX) return;
      const workspace = workspaceRef.current;
      if (!workspace) return;

      agentsResizeCleanupRef.current?.();
      event.preventDefault();

      const startX = event.clientX;
      const startWidth = agentsPanelWidthRef.current;
      const bodyStyle = document.body.style;
      const previousCursor = bodyStyle.cursor;
      const previousUserSelect = bodyStyle.userSelect;
      bodyStyle.cursor = "col-resize";
      bodyStyle.userSelect = "none";

      const cleanup = () => {
        window.removeEventListener("pointermove", onPointerMove);
        window.removeEventListener("pointerup", onPointerUp);
        window.removeEventListener("pointercancel", onPointerUp);
        bodyStyle.cursor = previousCursor;
        bodyStyle.userSelect = previousUserSelect;
        persistAgentsPanelWidthPreference(agentsPanelWidthRef.current);
        agentsResizeCleanupRef.current = null;
      };

      function onPointerMove(moveEvent: PointerEvent) {
        const workspaceWidth = workspace.getBoundingClientRect().width;
        const nextMaxWidth = resolveAgentsPanelMaxWidth(workspaceWidth);
        const nextWidth = clampAgentsPanelWidth(
          startWidth + (moveEvent.clientX - startX),
          nextMaxWidth
        );
        agentsPanelWidthRef.current = nextWidth;
        setAgentsPanelWidth(nextWidth);
      }

      function onPointerUp() {
        cleanup();
      }

      agentsResizeCleanupRef.current = cleanup;
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
      window.addEventListener("pointercancel", onPointerUp);
    },
    []
  );

  const handleSelectAgent = useCallback((id: string) => {
    setActiveAgent(id);
    setActiveSessionId(agentSessions[id] ?? null);
    setAgentsCollapsed(true);
  }, [agentSessions]);

  const onRespondPermission = useCallback(async (
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
      setError(parseApiErrorMessage(err) ?? String(err));
    } finally {
      setPermissionBusy(null);
    }
  }, [token]);

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
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  };

  useEffect(() => {
    if (auth?.role === "root" && isAdminRoute) {
      api.getAdminSettings(auth.token).then(res => {
        setPasskeyEnabled(res.passkey_enabled);
      }).catch(() => {});
    }
  }, [auth, isAdminRoute]);

  const onPasskeyEnabledChange = async (enabled: boolean) => {
    if (!auth || auth.role !== "root") return;
    try {
      await api.setPasskeyEnabled(auth.token, enabled);
      setPasskeyEnabled(enabled);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
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
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  };

  useEffect(() => {
    if (!developerMode && acpTab === "debug") {
      setAcpTab("conversation");
    }
  }, [acpTab, developerMode]);
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
        thinkingStartTs,
        runStatus: acpView.runStatus?.status ?? null,
        modelLabel: activeAgentModelLabel,
      }),
    [
      activeAgentRecord,
      activeSessionId,
      developerMode,
      acpView.hasAcp,
      acpView.runStatus?.status,
      thinkingStartTs,
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
        onSelectTab: handleAcpTabSelect,
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
      handleAcpTabSelect,
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
        setWorktreeMode: handleWorktreeModeChange,
        worktreeRepo,
        setWorktreeRepo,
        worktreeRef,
        setWorktreeRef,
        codeMode,
        setCodeMode,
        worktreeError,
        createBusy: createAgentBusy,
        workdirPlaceholder: selectedTargetNodeDefaultWorktreeRoot,
        onCreateAgent: onCreateAgent,
        onClose: closeCreateAgentModal,
      }),
    [
      agentName,
      agentWorkdir,
      agentPresetId,
      worktreeMode,
      worktreeRepo,
      worktreeRef,
      codeMode,
      worktreeError,
      createAgentBusy,
      selectedTargetNodeDefaultWorktreeRoot,
      onCreateAgent,
      closeCreateAgentModal,
      handleWorktreeModeChange,
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
              onCreateNode: onCreateAgentNode,
              onUpdateNode: onUpdateAgentNode,
              onDeleteNode: onDeleteAgentNode,
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
      nodeNameInput,
      nodeGrpcTargetInput,
      nodeTlsServerNameInput,
      nodeDefaultWorktreeRootInput,
      createAgentNodeBusy,
      updatingAgentNodeIds,
      deletingAgentNodeIds,
      onCreateAgentNode,
      onUpdateAgentNode,
      onDeleteAgentNode,
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

  if (routeLocation.pathname.startsWith("/join")) {
    return (
      <div className="app bg-white" ref={appRootRef}>
        <Suspense fallback={<RouteFallback label="Loading join flow..." />}>
          <LazyJoinPage onComplete={(next) => setAuth(next)} />
        </Suspense>
      </div>
    );
  }

  if (routeLocation.pathname.startsWith("/admin")) {
    if (!auth) {
      return <AuthRequiredGate />;
    }
    if (auth.role !== "root") {
      return <ForbiddenRoute />;
    }
    return (
      <div className="app bg-white" ref={appRootRef}>
        <Suspense fallback={<RouteFallback label="Loading admin console..." />}>
          <LazyAdminPage
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
            developerMode={developerMode}
            onDeveloperModeChange={handleDeveloperModeChange}
            passkeyEnabled={passkeyEnabled}
            onPasskeyEnabledChange={onPasskeyEnabledChange}
          />
        </Suspense>
      </div>
    );
  }

  if (isTeamsRoute(routeLocation.pathname)) {
    if (shouldRedirectTeamsToLogin(routeLocation.pathname, auth, token)) {
      return <AuthRedirect />;
    }
    const teamRoute = resolveTeamRoute(routeLocation.pathname);
    return (
      <div className="app bg-white" ref={appRootRef}>
        <Suspense fallback={<RouteFallback label="Loading teams..." />}>
          <LazyTeamPage
            auth={auth}
            token={token}
            onLogout={onLogout}
            developerMode={developerMode}
            routeTeamId={teamRoute?.teamId ?? null}
            defaultWorktreeRoot={defaultWorktreeRoot}
          />
        </Suspense>
      </div>
    );
  }

  if (postAuthRedirectTarget) {
    return <PostLoginRedirect target={postAuthRedirectTarget} />;
  }

  return (
    <AgentsRootPage
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
    />
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

export function parseSendInputSessionMismatch(
  message: string
): { expected: string; running: string } | null {
  const match = message.match(
    /agent session mismatch:\s*expected=([^\s]+)\s+running=([^\s]+)/
  );
  if (!match) return null;
  const expected = match[1]?.trim();
  const running = match[2]?.trim();
  if (!expected || !running) return null;
  return { expected, running };
}

function createAnsiRenderer(): (input: string) => string {
  const escapeAnsiHtml = (input: string): string =>
    input
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
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
      const safe = escapeAnsiHtml(text);
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

function isSamePendingPermissionCountMap(
  a: Record<string, number>,
  b: Record<string, number>
): boolean {
  const aKeys = Object.keys(a);
  const bKeys = Object.keys(b);
  if (aKeys.length !== bKeys.length) return false;
  for (const key of aKeys) {
    if (a[key] !== b[key]) return false;
  }
  return true;
}
