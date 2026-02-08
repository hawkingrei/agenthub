import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import QRCode from "qrcode";
import {
  api,
  AgentEvent,
  AgentRecord,
  AuditRecord,
  AcpPermissionRecord,
  DeviceRecord,
  SafePath,
  VapidInfo,
} from "./api";
import { buildAcpView } from "./acp";
import {
  shouldIgnoreAgentWsError,
  shouldOpenAgentSocket,
  sanitizeAgentError,
  isAgentActiveStatus,
} from "./agent_ws";
import { ErrorBanner } from "./error_banner";
import { clearAuthAndRedirect, isInvalidTokenMessage } from "./auth_redirect";
import {
  getAdaptivePollInterval,
  getMaxEventCursor,
  isCursorNewer,
  updateLastEventCursor,
} from "./event_polling";
import {
  appendOutputLine,
  buildAcpCacheSlice,
  isSameOutputList,
  mergeOutputs,
  OutputLine,
  selectCachedOutputs,
} from "./output_cache";
import { isNearBottom } from "./scroll";
import { escapeHtml } from "./markdown";
import { AgentsPanel } from "./components/agents_panel";
import { CreateAgentModal } from "./components/create_agent_modal";
import { InputDock } from "./components/input_dock";
import { OutputHeader } from "./components/output_header";
import { OutputBody } from "./components/output_body";
import { OutputErrorBoundary } from "./components/output_error_boundary";
import { PermissionModal } from "./components/permission_modal";
import { useAcpConversation } from "./hooks/use_acp_conversation";
import { loadOutputCaches, saveOutputCaches } from "./storage/output_cache_storage";
import { AdminPage } from "./pages/admin_page";
import { AuthRequired, ForbiddenPage } from "./pages/auth_pages";
import { JoinPage } from "./pages/join_page";
import { ensurePushSubscription } from "./push";
import {
  loginCredentialToJson,
  publicKeyCredentialCreationOptionsFromJson,
  publicKeyCredentialRequestOptionsFromJson,
  registerCredentialToJson,
} from "./webauthn";
import { AuthState } from "./types";

export function App() {
  const eventLimit = 200;
  const maxCachedEvents = 800;
  const maxCachedSessions = 40;
  const [auth, setAuth] = useState<AuthState | null>(() => {
    const raw = localStorage.getItem("agenthub_auth");
    return raw ? (JSON.parse(raw) as AuthState) : null;
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
  const [agentCommand, setAgentCommand] = useState("agenthub-codex-acp");
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
  const loadSeq = useRef(0);
  const isComposingRef = useRef(false);
  const [showCreateAgent, setShowCreateAgent] = useState(false);
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>(
    []
  );
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const ansi = useMemo(() => createAnsiRenderer(), []);
  const [input, setInput] = useState("");
  const sseRef = useRef<EventSource | null>(null);
  const outputRef = useRef<HTMLDivElement | null>(null);
  const lastAcpEventTsRef = useRef<number | null>(null);
  const [eventMeta, setEventMeta] = useState<
    Record<
      string,
      { oldestSeq: number | null; hasMore: boolean; loading: boolean; loaded: boolean }
    >
  >({});
  const [agentsCollapsed, setAgentsCollapsed] = useState(false);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [acpTab, setAcpTab] = useState<"conversation" | "debug">(
    "conversation"
  );
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<
    AcpPermissionRecord[]
  >([]);
  const [thinkingTick, setThinkingTick] = useState(0);
  const lastEventCursorRef = useRef<
    Record<string, { value: number; hasSeq: boolean }>
  >({});
  const fallbackSeqRef = useRef<number>(0);
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
  const acpView = useMemo(
    () => buildAcpView(acpOutputs),
    [acpOutputs, thinkingTick]
  );
  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );
  const activeAgentStatus = activeAgentRecord?.status ?? null;
  const isAgentActive = isAgentActiveStatus(activeAgentStatus);
  const showAcpRuntime = isAgentActive;
  const thinkingStartTs =
    activeAgentStatus === "running" ? acpView.thinkingStartTs : null;
  const canControlAcp = Boolean(activeAgent && isAgentActive);
  const activeEventKey = activeAgent
    ? `${activeAgent}:${activeSessionId ?? "latest"}`
    : null;
  const isOutputLoading =
    Boolean(activeEventKey) && eventMeta[activeEventKey]?.loaded !== true;

  const token = auth?.token ?? null;
  const refreshAgents = async () => {
    if (!token) return;
    try {
      const items = await api.listAgents(token);
      setAgents(items);
    } catch (err) {
      setError(formatWorktreeError(err) ?? String(err));
    }
  };

  useEffect(() => {
    if (!token) return;
    refreshAgents();
  }, [token]);

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

  const loadAgentEvents = async (
    id: string,
    sessionId?: string | null
  ): Promise<boolean> => {
    if (!token) return false;
    const seq = ++loadSeq.current;
    const key = `${id}:${sessionId ?? "latest"}`;
    const latestKey = `${id}:latest`;
    setEventMeta((prev) => {
      const current = prev[key];
      if (current?.loading) return prev;
      return {
        ...prev,
        [key]: {
          oldestSeq: current?.oldestSeq ?? null,
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
          if (activeAgent === id && !activeSessionId) {
            setActiveSessionId(latestSession);
          }
          return true;
        }
      }
      const ordered = [...events].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
      const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
      lastAcpEventTsRef.current = getLastAcpEventTs(acpOrdered);
      let next: AgentEvent[] = [];
      let combined: AgentEvent[] = [];
      let acpNext: AgentEvent[] = [];
      let acpCombined: AgentEvent[] = [];
      setOutputCache((prev) => {
        const existing = prev[key] ?? [];
        const merged = mergeOutputs(existing, ordered);
        const nextSlice =
          merged.length > maxCachedEvents
            ? merged.slice(merged.length - maxCachedEvents)
            : merged;
        next = nextSlice;
        if (key === latestKey) {
          combined = nextSlice;
        } else {
          const latest = prev[latestKey] ?? [];
          combined = mergeOutputs(nextSlice, latest);
        }
        if (isSameOutputList(existing, nextSlice)) return prev;
        return { ...prev, [key]: nextSlice };
      });
      setAcpOutputCache((prev) => {
        const existing = prev[key] ?? [];
        const nextSlice = buildAcpCacheSlice(
          existing,
          ordered,
          maxCachedEvents
        );
        acpNext = nextSlice;
        if (key === latestKey) {
          acpCombined = nextSlice;
        } else {
          const latest = prev[latestKey] ?? [];
          acpCombined = mergeOutputs(nextSlice, latest);
        }
        if (isSameOutputList(existing, nextSlice)) return prev;
        return { ...prev, [key]: nextSlice };
      });
      const oldestSeq = next.length ? next[0].seq ?? null : null;
      const nextOutputs = combined.length > 0 ? combined : next;
      const nextAcpOutputs = acpCombined.length > 0 ? acpCombined : acpNext;
      setOutputs((prev) =>
        isSameOutputList(prev, nextOutputs) ? prev : nextOutputs
      );
      setAcpOutputs((prev) =>
        isSameOutputList(prev, nextAcpOutputs) ? prev : nextAcpOutputs
      );
      let hasNew = false;
      const maxCursor = getMaxEventCursor(ordered);
      if (maxCursor != null) {
        const prevCursor = lastEventCursorRef.current[key];
        lastEventCursorRef.current[key] = maxCursor;
        hasNew = prevCursor == null ? true : isCursorNewer(prevCursor, maxCursor);
      }
      setEventMeta((prev) => {
        const nextMeta = {
          oldestSeq,
          hasMore: ordered.length >= eventLimit,
          loading: false,
          loaded: true,
        };
        const current = prev[key];
        if (
          current &&
          current.oldestSeq === nextMeta.oldestSeq &&
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
  };

  const loadOlderEvents = useCallback(async () => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore || meta.oldestSeq == null) {
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
        meta.oldestSeq
      );
      const ordered = [...older].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
      const acpOrdered = ordered.filter((evt) => evt.stream === "acp");
      const nextOldest = ordered.length ? ordered[0].seq ?? null : meta.oldestSeq;
      const hasMore = ordered.length >= eventLimit;
      setOutputs((prev) => mergeOutputs(prev, ordered));
      setAcpOutputs((prev) => mergeOutputs(prev, acpOrdered));
      setOutputCache((prev) => {
        const existing = prev[key] ?? [];
        const merged = mergeOutputs(existing, ordered);
        const trimmed =
          maxCachedEvents > 0
            ? merged.slice(Math.max(0, merged.length - maxCachedEvents))
            : merged;
        return { ...prev, [key]: trimmed };
      });
      setAcpOutputCache((prev) => {
        const existing = prev[key] ?? [];
        const merged = mergeOutputs(existing, acpOrdered);
        const trimmed =
          maxCachedEvents > 0
            ? merged.slice(Math.max(0, merged.length - maxCachedEvents))
            : merged;
        return { ...prev, [key]: trimmed };
      });
      setEventMeta((prev) => ({
        ...prev,
        [key]: { oldestSeq: nextOldest, hasMore, loading: false, loaded: true },
      }));
    } catch {
      setEventMeta((prev) => ({
        ...prev,
        [key]: { ...meta, loading: false, loaded: true },
      }));
    }
  }, [token, activeAgent, activeSessionId, eventMeta, eventLimit]);

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
    const selection = selectCachedOutputs(
      outputCache,
      acpOutputCache,
      key,
      latestKey
    );
    if (selection.source === "none") {
      setOutputs([]);
      setAcpOutputs([]);
      loadAgentEvents(activeAgent, activeSessionId);
      return;
    }
    const baseOutputs = selection.outputs ?? [];
    const baseAcpOutputs = selection.acpOutputs ?? [];
    const combinedOutputs =
      selection.source === "session" &&
      activeSessionId &&
      key !== latestKey &&
      baseOutputs.length > 0
        ? mergeOutputs(baseOutputs, outputCache[latestKey] ?? [])
        : baseOutputs;
    const combinedAcpOutputs =
      selection.source === "session" &&
      activeSessionId &&
      key !== latestKey &&
      baseAcpOutputs.length > 0
        ? mergeOutputs(baseAcpOutputs, acpOutputCache[latestKey] ?? [])
        : baseAcpOutputs;
    setOutputs((prev) =>
      isSameOutputList(prev, combinedOutputs) ? prev : combinedOutputs
    );
    setAcpOutputs((prev) =>
      isSameOutputList(prev, combinedAcpOutputs) ? prev : combinedAcpOutputs
    );
    if (!eventMeta[key]) {
      const oldestSeq = combinedOutputs.length
        ? combinedOutputs[0].seq ?? null
        : combinedAcpOutputs.length
          ? combinedAcpOutputs[0].seq ?? null
          : null;
      setEventMeta((prev) => ({
        ...prev,
        [key]: {
          oldestSeq,
          hasMore:
            combinedOutputs.length + combinedAcpOutputs.length >= eventLimit,
          loading: false,
          loaded: true,
        },
      }));
    }
  }, [token, activeAgent, activeSessionId]);

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
    let cancelled = false;
    loadAgentEvents(activeAgent, activeSessionId);
    if (!shouldOpenAgentSocket(activeAgentStatus)) return;
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
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (cancelled) return;
        openSource();
      }, delay);
    };
    const nextFallbackSeq = (payloadSeq: unknown) => {
      if (typeof payloadSeq === "number") {
        fallbackSeqRef.current = Math.max(fallbackSeqRef.current, payloadSeq);
        return payloadSeq;
      }
      const now = Date.now() * 1000;
      const next = Math.max(now, fallbackSeqRef.current + 1);
      fallbackSeqRef.current = next;
      return next;
    };
    const openSource = () => {
      if (cancelled) return;
      const source = new EventSource(
        `${location.origin}/sse/agents/${activeAgent}?token=${token}`
      );
      sseRef.current = source;
      source.onopen = () => {
        reconnectAttempt = 0;
      };
      source.onmessage = (event) => {
        if (event.data === "heartbeat") return;
        try {
          const parsed = JSON.parse(event.data);
          if (parsed.type === "output" || parsed.type === "acp") {
            const payload = parsed.payload;
            if (payload.agent_id && payload.agent_id !== activeAgent) {
              return;
            }
            if (
              activeSessionId &&
              payload.session_id &&
              payload.session_id !== activeSessionId
            ) {
              return;
            }
            const seq = nextFallbackSeq(payload.seq);
            const line: OutputLine = {
              ts: payload.ts,
              stream: payload.stream,
              message: payload.message,
              agent_id: payload.agent_id,
              session_id: payload.session_id,
              seq,
            };
            const key = `${payload.agent_id}:${payload.session_id ?? "latest"}`;
            updateLastEventCursor(lastEventCursorRef, key, line);
            if (payload.stream === "acp") {
              lastAcpEventTsRef.current = payload.ts;
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
            }
            setOutputs((prev) => appendOutputLine(prev, line));
            setOutputCache((prev) => ({
              ...prev,
              [key]: appendOutputLine(prev[key] ?? [], line),
            }));
            if (line.stream === "acp") {
              setAcpOutputs((prev) => appendOutputLine(prev, line));
              setAcpOutputCache((prev) => ({
                ...prev,
                [key]: appendOutputLine(prev[key] ?? [], line),
              }));
            }
          }
        } catch {
          if (typeof event.data === "string") {
            if (isInvalidTokenMessage(event.data)) {
              clearAuthAndRedirect();
              return;
            }
            if (shouldIgnoreAgentWsError(event.data, activeAgentStatus)) {
              return;
            }
            setError(event.data);
          }
        }
      };
      source.onerror = () => {
        if (sseRef.current !== source) {
          source.close();
          return;
        }
        source.close();
        sseRef.current = null;
        scheduleReconnect();
      };
    };
    openSource();
    const schedulePoll = (delay: number) => {
      if (cancelled) return;
      if (eventPollRef.current.timer) {
        window.clearTimeout(eventPollRef.current.timer);
      }
      const now = Date.now();
      const boostUntil = eventPollRef.current.boostUntil;
      const boostActive = boostUntil != null && boostUntil > now;
      const nextDelay = boostActive ? 1000 : delay;
      eventPollRef.current.timer = window.setTimeout(async () => {
        if (cancelled) return;
        const current = sseRef.current;
        const isOpen =
          current != null && current.readyState === EventSource.OPEN;
        let hasNew = false;
        if (!isOpen) {
          hasNew =
            (await loadAgentEvents(activeAgent, activeSessionId)) === true;
        } else {
          eventPollRef.current.idleCount = 0;
        }
        if (hasNew) {
          eventPollRef.current.idleCount = 0;
        } else if (!isOpen) {
          eventPollRef.current.idleCount += 1;
        }
        if (!cancelled) {
          schedulePoll(getAdaptivePollInterval(eventPollRef.current.idleCount));
        }
      }, nextDelay);
    };
    schedulePollRef.current = schedulePoll;
    schedulePoll(getAdaptivePollInterval(0));
    return () => {
      cancelled = true;
      clearReconnectTimer();
      if (eventPollRef.current.timer) {
        window.clearTimeout(eventPollRef.current.timer);
        eventPollRef.current.timer = null;
      }
      eventPollRef.current.idleCount = 0;
      eventPollRef.current.boostUntil = null;
      schedulePollRef.current = null;
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
    };
  }, [token, activeAgent, activeSessionId, activeAgentStatus]);

  useEffect(() => {
    const el = outputRef.current;
    if (!el) return;
    if (isNearBottom(el.scrollHeight, el.scrollTop, el.clientHeight)) {
      el.scrollTop = el.scrollHeight;
    }
  }, [outputs, acpView.hasAcp]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    let cancelled = false;
    const pollOnce = async (): Promise<number> => {
      try {
        const items = await api.listAcpPermissions(token, activeAgent, "pending");
        if (!cancelled) {
          setAcpPermissions((prev) =>
            isSamePermissionList(prev, items) ? prev : items
          );
        }
        return items.length;
      } catch {
        if (!cancelled) setAcpPermissions([]);
        return 0;
      }
    };
    const schedule = (delay: number) => {
      if (permissionPollRef.current.timer) {
        window.clearTimeout(permissionPollRef.current.timer);
      }
      permissionPollRef.current.timer = window.setTimeout(async () => {
        const pendingCount = await pollOnce();
        const nextDelay = pendingCount > 0 ? 5_000 : 3_000;
        schedule(nextDelay);
      }, delay);
    };
    schedulePermissionPollRef.current = schedule;
    schedule(0);
    return () => {
      cancelled = true;
      if (permissionPollRef.current.timer) {
        window.clearTimeout(permissionPollRef.current.timer);
        permissionPollRef.current.timer = null;
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
    const load = async () => {
      try {
        const items = await api.listAcpPermissions(token, activeAgent);
        if (!cancelled) {
          setAcpPermissionHistory((prev) =>
            isSamePermissionList(prev, items) ? prev : items
          );
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
      localStorage.setItem("agenthub_auth", JSON.stringify(next));
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
      localStorage.setItem("agenthub_auth", JSON.stringify(next));
      setAuth(next);
      await ensurePushSubscription(finish.token);
    } catch (err) {
      setError(String(err));
    }
  };

  const onLogout = () => {
    localStorage.removeItem("agenthub_auth");
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
  };

  const onCreateAgent = async () => {
    if (!token) return;
    setError(null);
    setWorktreeError(null);
    try {
      const name = agentName.trim() || "agent";
      const workdir = agentWorkdir.trim();
      const command = agentCommand.trim();
      const args: string[] = [];
      if (!workdir) {
        setError("workdir is required");
        return;
      }
      if (worktreeMode !== "use_existing" && !worktreeRepo.trim()) {
        setError("worktree repo is required");
        return;
      }
      const agent = await api.createAgent(token, {
        name,
        workdir,
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
      setAgentCommand("agenthub-codex-acp");
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
    }
  };

  const onStartAgent = async (id: string) => {
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
      const hint = formatWorktreeError(err);
      setError(hint ?? parseApiErrorMessage(err) ?? String(err));
    }
  };

  const onStopAgent = async (id: string) => {
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
  };

  const onDeleteAgent = async (id: string) => {
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
  };

  const onSetCodeMode = async (id: string, next: boolean) => {
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
  };

  const onAcpSetMode = async () => {
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
  };

  const onAcpSetModel = async () => {
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
  };

  const onAcpSetConfig = async () => {
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
  };

  const onAcpCancel = async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.cancelAcp(token, activeAgent);
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  };

  const onAcpClearSession = async () => {
    if (!token || !activeAgent) return;
    setError(null);
    try {
      await api.clearAcpSession(token, activeAgent, "codex");
    } catch (err) {
      setError(parseApiErrorMessage(err) ?? String(err));
    }
  };

  const onSendInput = async () => {
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
      const localSeq = Date.now() * 1_000_000;
      const line: OutputLine = {
        agent_id: activeAgent,
        session_id: activeSessionId,
        ts: Math.floor(Date.now() / 1000),
        seq: localSeq,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text,
          chunk: false,
          message_id: messageId,
        }),
      };
      setOutputs((prev) => mergeOutputs(prev, [line]));
      const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
      setOutputCache((prev) => ({
        ...prev,
        [key]: mergeOutputs(prev[key] ?? [], [line]),
      }));
      setAcpOutputs((prev) => mergeOutputs(prev, [line]));
      setAcpOutputCache((prev) => ({
        ...prev,
        [key]: mergeOutputs(prev[key] ?? [], [line]),
      }));
    }
    try {
      await api.sendInput(token, activeAgent, text, messageId ?? undefined);
      setInput("");
    } catch (err) {
      setError(String(err || "websocket not connected"));
    }
  };

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
      const qr = await QRCode.toDataURL(url);
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
        error={error}
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

  return (
    <div className="app">
      <header>
        <h1>AgentHub</h1>
        {auth && (
          <div className="session">
            <span>{auth.username}</span>
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

      {error && <ErrorBanner message={error} onClose={() => setError(null)} />}

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
        <section className={agentsCollapsed ? "workspace collapsed" : "workspace"}>
          <AgentsPanel
            agents={agents}
            activeAgent={activeAgent}
            agentsCollapsed={agentsCollapsed}
            onCollapse={() => setAgentsCollapsed(true)}
            onCreateAgent={() => setShowCreateAgent(true)}
            onSelectAgent={(id) => {
              setActiveAgent(id);
              setActiveSessionId(agentSessions[id] ?? null);
            }}
            onToggleCodeMode={onSetCodeMode}
            onStartAgent={onStartAgent}
            onStopAgent={onStopAgent}
            onDeleteAgent={onDeleteAgent}
          />
          <div className="workspace-right">
            <OutputHeader
              activeAgent={activeAgentRecord}
              agentsCollapsed={agentsCollapsed}
              onToggleAgents={() => setAgentsCollapsed((prev) => !prev)}
            />
            {activeAgent ? (
              <OutputErrorBoundary>
                <OutputBody
                  outputRef={outputRef}
                  isOutputLoading={isOutputLoading}
                  outputs={outputs}
                  ansi={ansi}
                  acpPanelProps={{
                    acpView,
                    activeSessionId,
                    showAcpRuntime,
                    thinkingStartTs,
                    acpTab,
                    onSelectTab: (next) => setAcpTab(next),
                    showConversationBadge: acpConversation.showConversationBadge,
                    conversation: {
                      items: acpConversation.conversationRenderItems,
                      windowOffset: acpConversation.conversationWindowOffset,
                      isFrozenView: acpConversation.isFrozenView,
                      shouldAutoCollapse: acpConversation.shouldAutoCollapse,
                      collapseCutoff: acpConversation.collapseCutoff,
                      stickToBottom: acpConversation.conversationStickToBottom,
                      pendingCount: acpConversation.conversationPendingCount,
                      avgHeight: acpConversation.conversationAvgHeight,
                      onScroll: acpConversation.handleConversationScroll,
                      containerRef: acpConversation.acpConversationRef,
                      ansi,
                    },
                    debug: {
                      currentMode: acpView.currentMode,
                      rawEvents: acpView.rawEvents,
                      acpPermissionHistory,
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
                    },
                  }}
                />
              </OutputErrorBoundary>
            ) : null}
            <InputDock
              input={input}
              onInputChange={setInput}
              onSendInput={onSendInput}
              onJumpToBottom={acpConversation.jumpToConversationBottom}
              showConversationJump={acpConversation.showConversationJump}
              isComposingRef={isComposingRef}
            />
          </div>
        </section>
      )}

      {auth && showCreateAgent && (
        <CreateAgentModal
          agentName={agentName}
          setAgentName={setAgentName}
          agentWorkdir={agentWorkdir}
          setAgentWorkdir={setAgentWorkdir}
          agentCommand={agentCommand}
          setAgentCommand={setAgentCommand}
          worktreeMode={worktreeMode}
          setWorktreeMode={setWorktreeMode}
          worktreeRepo={worktreeRepo}
          setWorktreeRepo={setWorktreeRepo}
          worktreeRef={worktreeRef}
          setWorktreeRef={setWorktreeRef}
          codeMode={codeMode}
          setCodeMode={setCodeMode}
          worktreeError={worktreeError}
          onCreateAgent={onCreateAgent}
          onClose={() => setShowCreateAgent(false)}
        />
      )}

      {auth && activeAgent && acpPermissions.length > 0 && (
        <PermissionModal
          permissions={acpPermissions}
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

function statusToAgentStatus(status: string): AgentRecord["status"] {
  if (status === "running") return "running";
  if (status === "idle") return "idle";
  if (status === "failed") return "failed";
  if (status === "completed" || status === "cancelled") return "stopped";
  return "stopped";
}

function getLastAcpEventTs(events: OutputLine[]): number | null {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const evt = events[i];
    if (evt.stream === "acp") return evt.ts;
  }
  return null;
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
