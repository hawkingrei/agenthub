import React, { useEffect, useMemo, useRef, useState } from "react";
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
} from "./agent_ws";
import { ErrorBanner } from "./error_banner";
import { clearAuthAndRedirect, isInvalidTokenMessage } from "./auth_redirect";
import {
  buildConversationMessages,
  ConversationItem,
  windowConversation,
} from "./conversation";
import { isNearBottom } from "./scroll";
import { renderMarkdown } from "./markdown";

type AuthState = {
  token: string;
  userId: string;
  username: string;
  role: string;
};

type OutputLine = AgentEvent;



export function App() {
  const eventLimit = 200;
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
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [agentSessions, setAgentSessions] = useState<Record<string, string>>(
    {}
  );
  const [outputCache, setOutputCache] = useState<
    Record<string, OutputLine[]>
  >({});
  const loadSeq = useRef(0);
  const isComposingRef = useRef(false);
  const [showCreateAgent, setShowCreateAgent] = useState(false);
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>(
    []
  );
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const ansi = useMemo(() => createAnsiRenderer(), []);
  const [input, setInput] = useState("");
  const wsRef = useRef<WebSocket | null>(null);
  const outputRef = useRef<HTMLDivElement | null>(null);
  const acpConversationRef = useRef<HTMLDivElement | null>(null);
  const acpStickToBottomRef = useRef(true);
  const pendingScrollAdjustRef = useRef<{
    prevHeight: number;
    prevTop: number;
  } | null>(null);
  const [eventMeta, setEventMeta] = useState<
    Record<string, { oldestSeq: number | null; hasMore: boolean; loading: boolean }>
  >({});
  const [agentsCollapsed, setAgentsCollapsed] = useState(false);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [acpTab, setAcpTab] = useState<
    "conversation" | "tools" | "plan" | "commands" | "debug"
  >("conversation");
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<
    AcpPermissionRecord[]
  >([]);
  const [thinkingTick, setThinkingTick] = useState(0);
  const [conversationStickToBottom, setConversationStickToBottom] = useState(true);
  const acpView = useMemo(
    () => buildAcpView(outputs),
    [outputs, thinkingTick]
  );
  const conversationMessages = useMemo<ConversationItem[]>(
    () => buildConversationMessages(acpView.messages, activeSessionId),
    [acpView.messages, activeSessionId]
  );
  const conversationWindow = useMemo(
    () => windowConversation(conversationMessages, conversationStickToBottom, 200),
    [conversationMessages, conversationStickToBottom]
  );
  useEffect(() => {
    if (acpTab !== "conversation") return;
    if (!activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore) return;
    const minMessages = 12;
    if (conversationMessages.length >= minMessages) return;
    loadOlderEvents();
  }, [conversationMessages.length, acpTab, activeAgent, activeSessionId, eventMeta]);
  useEffect(() => {
    if (acpTab !== "conversation") return;
    const el = acpConversationRef.current;
    if (!el) return;
    if (!conversationStickToBottom) return;
    el.scrollTop = el.scrollHeight;
  }, [conversationWindow.items.length, acpTab, conversationStickToBottom]);
  useEffect(() => {
    const el = acpConversationRef.current;
    const pending = pendingScrollAdjustRef.current;
    if (!el || !pending) return;
    const nextHeight = el.scrollHeight;
    el.scrollTop = nextHeight - pending.prevHeight + pending.prevTop;
    pendingScrollAdjustRef.current = null;
  }, [acpView.messages.length]);
  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );
  const activeAgentStatus = activeAgentRecord?.status ?? null;
  const showAcpRuntime = activeAgentStatus === "running";
  const canControlAcp = Boolean(activeAgent && activeAgentStatus === "running");

  const token = auth?.token ?? null;
  const mergeOutputs = (existing: OutputLine[], incoming: OutputLine[]) => {
    const merged = [...existing, ...incoming];
    const seen = new Set<string>();
    const deduped: OutputLine[] = [];
    for (const line of merged) {
      const key =
        line.seq != null
          ? String(line.seq)
          : `${line.ts}-${line.stream}-${line.message}`;
      if (seen.has(key)) continue;
      seen.add(key);
      deduped.push(line);
    }
    return deduped.sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
  };
  const appendOutputLine = (existing: OutputLine[], line: OutputLine) => {
    if (existing.length === 0) return [line];
    const lineSeq = line.seq ?? 0;
    const lastSeq = existing[existing.length - 1].seq ?? 0;
    if (lineSeq >= lastSeq) {
      return [...existing, line];
    }
    const next = existing.slice();
    let lo = 0;
    let hi = next.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      const midSeq = next[mid].seq ?? 0;
      if (midSeq <= lineSeq) {
        lo = mid + 1;
      } else {
        hi = mid;
      }
    }
    next.splice(lo, 0, line);
    return next;
  };
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
    const running = agents.find((agent) => agent.status === "running");
    const next = running ?? agents[0];
    if (next) {
      setActiveAgent(next.id);
      setActiveSessionId(agentSessions[next.id] ?? null);
    }
  }, [agents, activeAgent, agentSessions]);

  useEffect(() => {
    api.authStatus().then((res) => setRootInitialized(res.root_initialized)).catch(() => {});
  }, []);

  const loadAgentEvents = async (id: string, sessionId?: string | null) => {
    if (!token) return;
    const seq = ++loadSeq.current;
    try {
      const events = await api.listAgentEvents(
        token,
        id,
        eventLimit,
        sessionId ?? undefined
      );
      if (seq !== loadSeq.current) return;
      if (!sessionId) {
        const latestSession = [...events]
          .reverse()
          .find((evt) => evt.session_id)?.session_id;
        if (latestSession) {
          setAgentSessions((prev) => ({ ...prev, [id]: latestSession }));
          if (activeAgent === id && !activeSessionId) {
            setActiveSessionId(latestSession);
          }
          return;
        }
      }
      const key = `${id}:${sessionId ?? "latest"}`;
      const ordered = [...events].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
      const oldestSeq = ordered.length ? ordered[0].seq ?? null : null;
      setOutputCache((prev) => ({ ...prev, [key]: ordered }));
      setOutputs(ordered);
      setEventMeta((prev) => ({
        ...prev,
        [key]: {
          oldestSeq,
          hasMore: ordered.length >= eventLimit,
          loading: false,
        },
      }));
    } catch {
      // ignore
    }
  };

  const loadOlderEvents = async () => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const meta = eventMeta[key];
    if (!meta || meta.loading || !meta.hasMore || meta.oldestSeq == null) {
      return;
    }
    setEventMeta((prev) => ({
      ...prev,
      [key]: { ...meta, loading: true },
    }));
    const el = acpConversationRef.current;
    if (el) {
      pendingScrollAdjustRef.current = {
        prevHeight: el.scrollHeight,
        prevTop: el.scrollTop,
      };
    }
    try {
      const older = await api.listAgentEvents(
        token,
        activeAgent,
        eventLimit,
        activeSessionId ?? undefined,
        meta.oldestSeq
      );
      const ordered = [...older].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
      const nextOldest = ordered.length ? ordered[0].seq ?? null : meta.oldestSeq;
      const hasMore = ordered.length >= eventLimit;
      setOutputs((prev) => mergeOutputs(prev, ordered));
      setOutputCache((prev) => {
        const existing = prev[key] ?? [];
        return { ...prev, [key]: mergeOutputs(existing, ordered) };
      });
      setEventMeta((prev) => ({
        ...prev,
        [key]: { oldestSeq: nextOldest, hasMore, loading: false },
      }));
    } catch {
      setEventMeta((prev) => ({
        ...prev,
        [key]: { ...meta, loading: false },
      }));
    }
  };

  useEffect(() => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const cached = outputCache[key];
    if (cached) {
      setOutputs(cached);
      if (!eventMeta[key]) {
        const oldestSeq = cached.length ? cached[0].seq ?? null : null;
        setEventMeta((prev) => ({
          ...prev,
          [key]: {
            oldestSeq,
            hasMore: cached.length >= eventLimit,
            loading: false,
          },
        }));
      }
    } else {
      setOutputs([]);
      loadAgentEvents(activeAgent, activeSessionId);
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
    loadAgentEvents(activeAgent, activeSessionId);
    if (!shouldOpenAgentSocket(activeAgentStatus)) return;
    const ws = new WebSocket(
      `${location.origin.replace("http", "ws")}/ws/agents/${activeAgent}?token=${token}`
    );
    wsRef.current = ws;
    ws.onmessage = (event) => {
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
          const seq = payload.seq ?? Date.now();
          const line: OutputLine = {
            ts: payload.ts,
            stream: payload.stream,
            message: payload.message,
            agent_id: payload.agent_id,
            session_id: payload.session_id,
            seq,
          };
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
          }
          setOutputs((prev) => appendOutputLine(prev, line));
          const key = `${payload.agent_id}:${payload.session_id ?? "latest"}`;
          setOutputCache((prev) => ({
            ...prev,
            [key]: appendOutputLine(prev[key] ?? [], line),
          }));
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
    ws.onclose = () => {
      if (wsRef.current === ws) {
        wsRef.current = null;
      }
    };
    const poll = window.setInterval(() => {
      const current = wsRef.current;
      if (!current || current.readyState !== WebSocket.OPEN) {
        loadAgentEvents(activeAgent, activeSessionId);
      }
    }, 2000);
    return () => {
      window.clearInterval(poll);
      if (wsRef.current === ws) {
        wsRef.current = null;
      }
      ws.close();
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
    const load = async () => {
      try {
        const items = await api.listAcpPermissions(token, activeAgent, "pending");
        if (!cancelled) setAcpPermissions(items);
      } catch {
        if (!cancelled) setAcpPermissions([]);
      }
    };
    load();
    const timer = window.setInterval(load, 3000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [token, activeAgent, activeSessionId]);

  useEffect(() => {
    if (!acpView.thinkingStartTs) return;
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [acpView.thinkingStartTs]);

  useEffect(() => {
    if (!token || !activeAgent) return;
    let cancelled = false;
    const load = async () => {
      try {
        const items = await api.listAcpPermissions(token, activeAgent);
        if (!cancelled) setAcpPermissionHistory(items);
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
        setOutputs([]);
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
      setOutputs([]);
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
    }
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "input", data: text, message_id: messageId }));
      setInput("");
      return;
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
              <a className="icon-button" href="/admin" title="Admin">
                ⚙
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
          {!agentsCollapsed && (
            <div
              className="agents-backdrop"
              onClick={() => setAgentsCollapsed(true)}
            />
          )}
          <div className={agentsCollapsed ? "workspace-left collapsed" : "workspace-left"}>
            <div className="toolbar">
              <h2>Agents</h2>
              <div className="toolbar-actions">
                <button onClick={() => setShowCreateAgent(true)}>
                  Create Agent
                </button>
              </div>
            </div>
            <div className="agent-layout">
              <div className="agent-list">
                {agents.map((agent) => (
                  <div
                    key={agent.id}
                    className={
                      activeAgent === agent.id ? "agent-row active" : "agent-row"
                    }
                    role="button"
                    tabIndex={0}
                    onClick={() => {
                      setActiveAgent(agent.id);
                      setActiveSessionId(agentSessions[agent.id] ?? null);
                    }}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        setActiveAgent(agent.id);
                        setActiveSessionId(agentSessions[agent.id] ?? null);
                      }
                    }}
                    title={`ID: ${agent.id}\nWorkdir: ${agent.workdir}\nCommand: ${agent.command}\nStatus: ${agent.status}\nCode mode: ${agent.code_mode ? "on" : "off"}`}
                  >
                    <div className="agent-row-head">
                      <span className="agent-name">{agent.name}</span>
                      <div className="agent-row-actions">
                        <button
                          className={
                            agent.code_mode
                              ? "icon-button small code-active"
                              : "icon-button small"
                          }
                          onClick={(e) => {
                            e.stopPropagation();
                            onSetCodeMode(agent.id, !agent.code_mode);
                          }}
                          title={
                            agent.code_mode
                              ? "Disable code mode"
                              : "Enable code mode"
                          }
                          aria-pressed={agent.code_mode}
                        >
                          CM
                        </button>
                        <span className={`agent-status ${agent.status}`}>
                          {agent.status}
                        </span>
                        <button
                          className="icon-button small"
                          disabled={agent.status === "running"}
                          onClick={(e) => {
                            e.stopPropagation();
                            if (agent.status !== "running") {
                              onStartAgent(agent.id);
                            }
                          }}
                          title={
                            agent.status === "running"
                              ? "Already running"
                              : "Start"
                          }
                        >
                          ▶
                        </button>
                        {agent.status === "running" && (
                          <button
                            className="icon-button small"
                            onClick={(e) => {
                              e.stopPropagation();
                              onStopAgent(agent.id);
                            }}
                            title="Stop"
                          >
                            ⏹
                          </button>
                        )}
                        <button
                          className="icon-button small danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            onDeleteAgent(agent.id);
                          }}
                          title="Delete"
                        >
                          🗑
                        </button>
                      </div>
                    </div>
                    <div className="agent-row-meta">
                      <span>{agent.workdir}</span>
                      <span className="agent-code-mode">
                        Code mode: {agent.code_mode ? "on" : "off"}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="workspace-right">
            <div className="output-header">
              <div className="output-title">
                <button
                  className="icon-button small"
                  onClick={() => setAgentsCollapsed((prev) => !prev)}
                  title={agentsCollapsed ? "Show agents" : "Hide agents"}
                >
                  {agentsCollapsed ? "›" : "‹"}
                </button>
                <h2>Output</h2>
              </div>
              {activeAgentRecord && (
                <span className="output-subtitle">
                  {activeAgentRecord.name} · Code mode:{" "}
                  {activeAgentRecord.code_mode ? "on" : "off"}
                </span>
              )}
            </div>
            {activeAgent ? (
              <div className="output-body" ref={outputRef}>
                {acpView.hasAcp ? (
                  <div className="acp">
                    <div className="acp-head">
                      <div className="acp-title">
                        ACP
                        {activeSessionId && (
                          <span className="acp-session">
                            {activeSessionId.slice(0, 8)}
                          </span>
                        )}
                        {showAcpRuntime && acpView.runStatus?.status && (
                          <span
                            className={`acp-run ${acpView.runStatus.status}`}
                          >
                            {acpView.runStatus.status}
                          </span>
                        )}
                        {showAcpRuntime && acpView.thinkingStartTs && (
                          <span className="acp-thinking">
                            thinking{" "}
                            {Math.max(
                              0,
                              Math.floor(
                                Date.now() / 1000 - acpView.thinkingStartTs
                              )
                            )}
                            s
                          </span>
                        )}
                      </div>
                      <div className="acp-tabs">
                        <button
                          className={
                            acpTab === "conversation" ? "tab active" : "tab"
                          }
                          onClick={() => setAcpTab("conversation")}
                        >
                          Conversation
                        </button>
                        <button
                          className={acpTab === "tools" ? "tab active" : "tab"}
                          onClick={() => setAcpTab("tools")}
                        >
                          Tool Calls
                        </button>
                        <button
                          className={acpTab === "plan" ? "tab active" : "tab"}
                          onClick={() => setAcpTab("plan")}
                        >
                          Plan
                        </button>
                        <button
                          className={
                            acpTab === "commands" ? "tab active" : "tab"
                          }
                          onClick={() => setAcpTab("commands")}
                        >
                          Commands
                        </button>
                        <button
                          className={acpTab === "debug" ? "tab active" : "tab"}
                          onClick={() => setAcpTab("debug")}
                        >
                          Debug
                        </button>
                      </div>
                    </div>
                    {acpTab === "conversation" && (
                      <div
                        className="acp-conversation"
                        ref={acpConversationRef}
                        onScroll={() => {
                          const el = acpConversationRef.current;
                          if (!el) return;
                          const stick = isNearBottom(
                            el.scrollHeight,
                            el.scrollTop,
                            el.clientHeight
                          );
                          acpStickToBottomRef.current = stick;
                          setConversationStickToBottom(stick);
                          if (el.scrollTop < 80) {
                            loadOlderEvents();
                          }
                        }}
                      >
                        <div className="acp-conversation-inner">
                          {conversationWindow.items.map((msg, idx) => {
                            const key = `${conversationWindow.offset + idx}-${msg.kind}`;
                            if (msg.kind === "agent_thinking") {
                              return (
                                <div key={key} className="acp-bubble agent_thinking">
                                  <details className="acp-thought-fold" open={msg.live}>
                                    <summary>
                                      {msg.live
                                        ? "Thinking (live)"
                                        : "Thinking (collapsed)"}
                                    </summary>
                                    <div className="acp-text">
                                      <pre>{msg.text}</pre>
                                    </div>
                                  </details>
                                </div>
                              );
                            }
                            if (msg.kind === "agent_message") {
                              return (
                                <div key={key} className="acp-bubble agent_message">
                                  <div
                                    className="acp-text"
                                    dangerouslySetInnerHTML={{
                                      __html: renderMarkdown(msg.text),
                                    }}
                                  />
                                </div>
                              );
                            }
                            return (
                              <div key={key} className="acp-bubble user_message">
                                <div
                                  className="acp-text"
                                  dangerouslySetInnerHTML={{
                                    __html: renderMarkdown(msg.text),
                                  }}
                                />
                              </div>
                            );
                          })}
                        </div>
                      </div>
                    )}
                    {acpTab === "tools" && (
                      <div className="acp-tools">
                        {acpView.toolCalls.map((tool) => (
                          <div key={tool.id} className="acp-tool">
                            <div className="head">
                              <span>{tool.title}</span>
                              {tool.status && (
                                <span className={`status ${tool.status}`}>
                                  {tool.status}
                                </span>
                              )}
                            </div>
                            {tool.raw_input && (
                              <pre className="acp-content">
                                {JSON.stringify(tool.raw_input, null, 2)}
                              </pre>
                            )}
                            {tool.raw_output && (
                              <pre className="acp-content">
                                {JSON.stringify(tool.raw_output, null, 2)}
                              </pre>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                    {acpTab === "plan" && (
                      <div className="acp-plan">
                        {acpView.plan ? (
                          acpView.plan.entries.map((entry, idx) => (
                            <div key={idx} className="acp-plan-item">
                              <div className="title">{entry.content}</div>
                              {entry.status && (
                                <div className="meta">{entry.status}</div>
                              )}
                            </div>
                          ))
                        ) : (
                          <div className="empty">No plan available.</div>
                        )}
                      </div>
                    )}
                    {acpTab === "commands" && (
                      <div className="acp-commands">
                        {acpView.commands.map((cmd, idx) => (
                          <div key={idx} className="acp-command">
                            <div className="title">{cmd.name}</div>
                            {cmd.description && (
                              <div className="meta">{cmd.description}</div>
                            )}
                            {cmd.input && (
                              <pre className="acp-content">
                                {JSON.stringify(cmd.input, null, 2)}
                              </pre>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                    {acpTab === "debug" && (
                      <div className="acp-debug">
                        <div className="acp-controls">
                          <h4>Session Controls</h4>
                          <div className="acp-control-meta">
                            Current mode: {acpView.currentMode ?? "unknown"}
                          </div>
                          <div className="form-row">
                            <input
                              placeholder="Mode ID"
                              value={acpModeId}
                              onChange={(e) => setAcpModeId(e.target.value)}
                            />
                            <button onClick={onAcpSetMode} disabled={!canControlAcp}>
                              Set Mode
                            </button>
                          </div>
                          <div className="form-row">
                            <input
                              placeholder="Model ID"
                              value={acpModelId}
                              onChange={(e) => setAcpModelId(e.target.value)}
                            />
                            <button onClick={onAcpSetModel} disabled={!canControlAcp}>
                              Set Model
                            </button>
                          </div>
                          <div className="form-row">
                            <input
                              placeholder="Config ID"
                              value={acpConfigId}
                              onChange={(e) => setAcpConfigId(e.target.value)}
                            />
                            <input
                              placeholder="Config Value ID"
                              value={acpConfigValue}
                              onChange={(e) => setAcpConfigValue(e.target.value)}
                            />
                            <button onClick={onAcpSetConfig} disabled={!canControlAcp}>
                              Set Config
                            </button>
                          </div>
                          <div className="form-row">
                            <button onClick={onAcpCancel} disabled={!canControlAcp}>
                              Cancel Run
                            </button>
                            <button onClick={onAcpClearSession}>
                              Clear Session
                            </button>
                          </div>
                        </div>
                        <div className="acp-permissions">
                          <h4>Permissions</h4>
                          {acpPermissionHistory.length === 0 && (
                            <div className="empty">No permissions yet.</div>
                          )}
                          {acpPermissionHistory.map((perm) => (
                            <div key={perm.id} className="acp-permission">
                              <div className="head">
                                <div className="title">{perm.permission}</div>
                                <div className="meta">{perm.status}</div>
                              </div>
                            </div>
                          ))}
                        </div>
                        <div className="acp-raw-wrapper">
                          <h4>Raw Events</h4>
                          <ul className="acp-raw">
                            {acpView.rawEvents.map((evt, idx) => (
                              <li key={`${evt.ts}-${idx}`}>
                                <div className="meta">
                                  <span>
                                    {new Date(
                                      evt.ts * 1000
                                    ).toLocaleTimeString()}
                                  </span>
                                  <span className="mono">{evt.type}</span>
                                </div>
                                <pre className="acp-content">
                                  {JSON.stringify(evt.payload, null, 2)}
                                </pre>
                              </li>
                            ))}
                          </ul>
                        </div>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="terminal">
                    {outputs.map((line, idx) => (
                      <div
                        key={idx}
                        className={`line ${line.stream}`}
                        dangerouslySetInnerHTML={{
                          __html: ansi(line.message),
                        }}
                      />
                    ))}
                  </div>
                )}
              </div>
            ) : null}
            <div className="input docked">
              <textarea
                placeholder="Send input (Enter to send, Shift+Enter for newline)"
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onCompositionStart={() => {
                  isComposingRef.current = true;
                }}
                onCompositionEnd={() => {
                  isComposingRef.current = false;
                }}
                onKeyDown={(e) => {
                  if (
                    e.key === "Enter" &&
                    !e.shiftKey &&
                    !isComposingRef.current
                  ) {
                    e.preventDefault();
                    onSendInput();
                  }
                }}
                rows={2}
              />
              <button onClick={onSendInput}>Send</button>
            </div>
          </div>
        </section>
      )}

      {auth && showCreateAgent && (
        <div className="modal-backdrop">
          <div className="modal">
            <div className="modal-head">
              <h3>Create Agent</h3>
              <button
                className="ghost"
                onClick={() => setShowCreateAgent(false)}
              >
                Close
              </button>
            </div>
            <div className="modal-body">
              <div className="form-grid">
                <input
                  placeholder="Agent name"
                  value={agentName}
                  onChange={(e) => setAgentName(e.target.value)}
                />
                <input
                  placeholder="Workdir"
                  value={agentWorkdir}
                  onChange={(e) => setAgentWorkdir(e.target.value)}
                />
                <select
                  value={worktreeMode}
                  onChange={(e) =>
                    setWorktreeMode(
                      e.target.value as
                        | "use_existing"
                        | "create_worktree"
                        | "reuse_worktree"
                    )
                  }
                >
                  <option value="use_existing">Use existing workdir</option>
                  <option value="create_worktree">Create git worktree</option>
                  <option value="reuse_worktree">Reuse git worktree</option>
                </select>
                {(worktreeMode === "create_worktree" ||
                  worktreeMode === "reuse_worktree") && (
                  <input
                    placeholder="Worktree repo path"
                    value={worktreeRepo}
                    onChange={(e) => setWorktreeRepo(e.target.value)}
                  />
                )}
                {worktreeMode === "create_worktree" && (
                  <input
                    placeholder="Worktree ref (branch or commit)"
                    value={worktreeRef}
                    onChange={(e) => setWorktreeRef(e.target.value)}
                  />
                )}
                <select
                  value={agentCommand}
                  onChange={(e) => setAgentCommand(e.target.value)}
                >
                  <option value="agenthub-codex-acp">agenthub-codex-acp</option>
                </select>
              </div>
              <div className="checkbox-row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={codeMode}
                    onChange={(e) => setCodeMode(e.target.checked)}
                  />
                  <span>Code mode</span>
                </label>
              </div>
              {worktreeError && (
                <div className="worktree-error">
                  <h4>Worktree Setup Failed</h4>
                  <p>{worktreeError}</p>
                  <ul>
                    <li>Check Safe Paths for the workdir and repo path.</li>
                    <li>Ensure the workdir is empty when creating a worktree.</li>
                    <li>Verify the git repo exists and the ref is valid.</li>
                  </ul>
                </div>
              )}
            </div>
            <div className="modal-actions">
              <button onClick={onCreateAgent}>Create Agent</button>
              <button
                className="ghost"
                onClick={() => setShowCreateAgent(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {auth && activeAgent && acpPermissions.length > 0 && (
        <div className="modal-backdrop">
          <div className="modal">
            <div className="modal-head">
              <h3>Permission Requests</h3>
              <span className="badge">{acpPermissions.length}</span>
            </div>
            <div className="modal-body">
              {acpPermissions.map((perm) => {
                const toolCall = perm.tool_call as {
                  title?: string;
                  tool_call_id?: string;
                } | null;
                const title =
                  toolCall?.title ??
                  perm.tool_call_id ??
                  "Permission Request";
                return (
                  <div key={perm.id} className="acp-permission">
                    <div className="head">
                      <div className="title">{title}</div>
                      <div className="meta">{perm.status}</div>
                    </div>
                    <div className="options">
                      {perm.options.map((opt) => (
                        <button
                          key={opt.option_id}
                          disabled={permissionBusy === perm.id}
                          onClick={() =>
                            onRespondPermission(
                              perm.agent_id,
                              perm.id,
                              opt.option_id
                            )
                          }
                        >
                          {opt.name}
                        </button>
                      ))}
                      <button
                        disabled={permissionBusy === perm.id}
                        onClick={() =>
                          onRespondPermission(perm.agent_id, perm.id)
                        }
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function AuthRequired() {
  return (
    <div className="app">
      <section className="auth">
        <h2>Login Required</h2>
        <p>Please login to continue.</p>
      </section>
    </div>
  );
}

function ForbiddenPage() {
  return (
    <div className="app">
      <section className="auth">
        <h2>Forbidden</h2>
        <p>You do not have access to this page.</p>
      </section>
    </div>
  );
}

type AdminProps = {
  auth: AuthState;
  error: string | null;
  setError: (value: string | null) => void;
  safePaths: SafePath[];
  selectedSafePaths: Set<string>;
  onToggleSafePath: (path: string) => void;
  onToggleAllSafePaths: () => void;
  onDeleteSelectedSafePaths: () => void;
  devices: DeviceRecord[];
  audits: AuditRecord[];
  vapidInfo: VapidInfo | null;
  onRotateVapid: () => void;
  onAddSafePath: () => void;
  onDeleteSafePath: (path: string) => void;
  onRevokeDevice: (id: string) => void;
  onCreateJoin: () => void;
  joinQr: string | null;
  joinToken: string | null;
  joinPin: string | null;
  safePathInput: string;
  setSafePathInput: (value: string) => void;
};

function AdminPage(props: AdminProps) {
  const [tab, setTab] = useState<"safe" | "devices" | "audits" | "join" | "vapid">(
    "safe"
  );
  return (
    <div className="app">
      <header>
        <h1>AgentHub Admin</h1>
        <div className="session">
          <span>{props.auth.username}</span>
        </div>
      </header>

      {props.error && (
        <ErrorBanner message={props.error} onClose={() => props.setError(null)} />
      )}

      <section className="admin">
        <div className="toolbar">
          <h2>Admin</h2>
          <button onClick={props.onCreateJoin}>Create Join QR</button>
        </div>
        <div className="tab-bar">
          <button
            className={tab === "safe" ? "tab active" : "tab"}
            onClick={() => setTab("safe")}
          >
            Safe Paths
          </button>
          <button
            className={tab === "devices" ? "tab active" : "tab"}
            onClick={() => setTab("devices")}
          >
            Devices
          </button>
          <button
            className={tab === "audits" ? "tab active" : "tab"}
            onClick={() => setTab("audits")}
          >
            Login Audits
          </button>
          <button
            className={tab === "join" ? "tab active" : "tab"}
            onClick={() => setTab("join")}
          >
            Join Device
          </button>
          <button
            className={tab === "vapid" ? "tab active" : "tab"}
            onClick={() => setTab("vapid")}
          >
            VAPID Keys
          </button>
        </div>

        <div className="admin-panel">
          {tab === "safe" && (
            <div className="card">
              <h3>Safe Paths</h3>
              <div className="form-row">
                <input
                  placeholder="Add safe path"
                  value={props.safePathInput}
                  onChange={(e) => props.setSafePathInput(e.target.value)}
                />
                <button onClick={props.onAddSafePath}>Add Path</button>
              </div>
              <div className="form-row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={
                      props.safePaths.length > 0 &&
                      props.safePaths.every((p) =>
                        props.selectedSafePaths.has(p.path)
                      )
                    }
                    onChange={props.onToggleAllSafePaths}
                  />
                  Select All
                </label>
                <button onClick={props.onDeleteSelectedSafePaths}>
                  Delete Selected
                </button>
              </div>
              <ul>
                {props.safePaths.map((p) => (
                  <li key={p.path}>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={props.selectedSafePaths.has(p.path)}
                        onChange={() => props.onToggleSafePath(p.path)}
                      />
                    </label>
                    <span>{p.path}</span>
                    <button onClick={() => props.onDeleteSafePath(p.path)}>
                      Delete
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "devices" && (
            <div className="card">
              <h3>Devices</h3>
              <ul>
                {props.devices.map((d) => (
                  <li key={d.id}>
                    <span>
                      {d.name} - {d.status}
                    </span>
                    {d.status === "active" && (
                      <button onClick={() => props.onRevokeDevice(d.id)}>
                        Revoke
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "audits" && (
            <div className="card">
              <h3>Login Audits</h3>
              <ul>
                {props.audits.map((a) => (
                  <li key={a.id}>
                    <span>
                      {new Date(a.ts * 1000).toLocaleString()} - {a.event}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {tab === "join" && (
            <div className="card join-card">
              <h3>Join Device</h3>
              {props.joinQr && <img src={props.joinQr} alt="join qr" />}
              {props.joinToken && <p>Token: {props.joinToken}</p>}
              {props.joinPin && <p>PIN: {props.joinPin}</p>}
            </div>
          )}
          {tab === "vapid" && (
            <div className="card">
              <h3>VAPID Keys</h3>
              {props.vapidInfo ? (
                <div className="kv-list">
                  <div className="kv-row">
                    <span className="label">Subject</span>
                    <span className="value">{props.vapidInfo.subject}</span>
                  </div>
                  <div className="kv-row">
                    <span className="label">Public Key</span>
                    <span className="value mono">{props.vapidInfo.public_key}</span>
                  </div>
                  <div className="kv-row">
                    <span className="label">Keys Path</span>
                    <span className="value mono">{props.vapidInfo.keys_path}</span>
                  </div>
                </div>
              ) : (
                <p>VAPID keys not loaded.</p>
              )}
              {/* TODO: add copy button and rotate confirmation. */}
              <div className="form-row">
                <button onClick={props.onRotateVapid}>Rotate Keys</button>
              </div>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function JoinPage({ onComplete }: { onComplete: (auth: AuthState) => void }) {
  const token = new URLSearchParams(location.search).get("token") || "";
  const [tokenError] = useState(token ? null : "missing join token");
  const [pin, setPin] = useState("");
  const [username, setUsername] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [password, setPassword] = useState("");
  const [deviceName, setDeviceName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const onJoin = async () => {
    setError(null);
    try {
      const start = await api.joinStart({
        token,
        pin,
        username,
        display_name: displayName,
        password,
        device_name: deviceName || "Device",
      });
      const options = publicKeyCredentialCreationOptionsFromJson(start.options);
      const cred = await navigator.credentials.create({ publicKey: options });
      if (!cred) throw new Error("registration cancelled");
      const payload = registerCredentialToJson(cred as PublicKeyCredential);
      const finish = await api.joinFinish(start.challenge_id, payload);
      const next = {
        token: finish.token,
        userId: finish.user_id,
        username,
        role: "device",
      };
      localStorage.setItem("agenthub_auth", JSON.stringify(next));
      await ensurePushSubscription(finish.token);
      onComplete(next);
      location.href = "/";
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="app">
      <section className="auth">
        <h2>Join Device</h2>
        {tokenError && <div className="error">{tokenError}</div>}
        {error && <ErrorBanner message={error} onClose={() => setError(null)} />}
        <input placeholder="PIN" value={pin} onChange={(e) => setPin(e.target.value)} />
        <input
          placeholder="Username"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
        />
        <input
          placeholder="Display Name"
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
        />
        <input
          placeholder="Password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <input
          placeholder="Device Name"
          value={deviceName}
          onChange={(e) => setDeviceName(e.target.value)}
        />
        <button onClick={onJoin}>Join</button>
      </section>
    </div>
  );
}

async function ensurePushSubscription(token: string) {
  if (!("serviceWorker" in navigator)) return;
  if (!("PushManager" in window)) return;
  let registration: ServiceWorkerRegistration;
  try {
    registration = await navigator.serviceWorker.register("/sw.js");
    registration = await navigator.serviceWorker.ready;
  } catch {
    return;
  }
  if (!registration.pushManager) return;
  let sub = await registration.pushManager.getSubscription();
  if (!sub) {
    let vapid: { public_key: string };
    try {
      vapid = await api.getVapidPublicKey();
    } catch {
      return;
    }
    const key = urlBase64ToUint8Array(vapid.public_key);
    try {
      sub = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: key,
      });
    } catch {
      return;
    }
  }
  try {
    await api.subscribePush(token, sub.toJSON());
  } catch {
    return;
  }
}


function publicKeyCredentialCreationOptionsFromJson(options: unknown) {
  const maybe = options as { publicKey?: PublicKeyCredentialCreationOptions };
  const o = (maybe.publicKey ?? options) as PublicKeyCredentialCreationOptions;
  const challenge = toArrayBuffer(o.challenge, "challenge");
  const user = o.user as PublicKeyCredentialUserEntity;
  const userId = toArrayBuffer(user.id, "user.id");
  const exclude = (o.excludeCredentials ?? []).map((c) => ({
    ...c,
    id: toArrayBuffer(c.id, "excludeCredentials.id"),
  }));
  return {
    ...o,
    challenge,
    user: { ...user, id: userId },
    excludeCredentials: exclude,
  } as PublicKeyCredentialCreationOptions;
}

function publicKeyCredentialRequestOptionsFromJson(options: unknown) {
  const maybe = options as { publicKey?: PublicKeyCredentialRequestOptions };
  const o = (maybe.publicKey ?? options) as PublicKeyCredentialRequestOptions;
  const challenge = toArrayBuffer(o.challenge, "challenge");
  const allow = (o.allowCredentials ?? []).map((c) => ({
    ...c,
    id: toArrayBuffer(c.id, "allowCredentials.id"),
  }));
  return {
    ...o,
    challenge,
    allowCredentials: allow,
  } as PublicKeyCredentialRequestOptions;
}

function registerCredentialToJson(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAttestationResponse;
  return {
    id: cred.id,
    rawId: bufferToBase64Url(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: bufferToBase64Url(response.clientDataJSON),
      attestationObject: bufferToBase64Url(response.attestationObject),
      transports: response.getTransports?.() ?? [],
    },
  };
}

function loginCredentialToJson(cred: PublicKeyCredential) {
  const response = cred.response as AuthenticatorAssertionResponse;
  return {
    id: cred.id,
    rawId: bufferToBase64Url(cred.rawId),
    type: cred.type,
    response: {
      clientDataJSON: bufferToBase64Url(response.clientDataJSON),
      authenticatorData: bufferToBase64Url(response.authenticatorData),
      signature: bufferToBase64Url(response.signature),
      userHandle: response.userHandle
        ? bufferToBase64Url(response.userHandle)
        : null,
    },
  };
}

function bufferToBase64Url(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  bytes.forEach((b) => (binary += String.fromCharCode(b)));
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBuffer(value: string): ArrayBuffer {
  const base64 = value.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function urlBase64ToUint8Array(value: string): Uint8Array {
  return new Uint8Array(base64UrlToBuffer(value));
}

function toArrayBuffer(input: unknown, label: string): ArrayBuffer {
  if (!input) {
    throw new Error(`missing ${label}`);
  }
  if (typeof input === "string") {
    return base64UrlToBuffer(input);
  }
  if (input instanceof ArrayBuffer) {
    return input;
  }
  if (input instanceof Uint8Array) {
    return input.buffer;
  }
  if (Array.isArray(input)) {
    return new Uint8Array(input).buffer;
  }
  throw new Error(`unsupported ${label} type`);
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
  if (status === "failed") return "failed";
  if (status === "completed" || status === "cancelled") return "stopped";
  return "stopped";
}
