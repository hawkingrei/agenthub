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

type AuthState = {
  token: string;
  userId: string;
  username: string;
  role: string;
};

type OutputLine = AgentEvent;



export function App() {
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
  const [codeMode, setCodeMode] = useState(false);
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
  const [showCreateAgent, setShowCreateAgent] = useState(false);
  const [acpPermissions, setAcpPermissions] = useState<AcpPermissionRecord[]>(
    []
  );
  const [permissionBusy, setPermissionBusy] = useState<string | null>(null);
  const ansi = useMemo(() => createAnsiRenderer(), []);
  const [input, setInput] = useState("");
  const wsRef = useRef<WebSocket | null>(null);
  const outputRef = useRef<HTMLDivElement | null>(null);
  const [rootInitialized, setRootInitialized] = useState<boolean | null>(null);
  const [acpTab, setAcpTab] = useState<
    "conversation" | "tools" | "plan" | "commands" | "debug"
  >("conversation");
  const [acpPermissionHistory, setAcpPermissionHistory] = useState<
    AcpPermissionRecord[]
  >([]);
  const [thinkingTick, setThinkingTick] = useState(0);
  const acpView = useMemo(
    () => buildAcpView(outputs),
    [outputs, thinkingTick]
  );
  const activeAgentRecord = useMemo(
    () => agents.find((agent) => agent.id === activeAgent) ?? null,
    [agents, activeAgent]
  );

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
    api.authStatus().then((res) => setRootInitialized(res.root_initialized)).catch(() => {});
  }, []);

  const loadAgentEvents = async (id: string, sessionId?: string | null) => {
    if (!token) return;
    const seq = ++loadSeq.current;
    try {
      const events = await api.listAgentEvents(
        token,
        id,
        500,
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
        }
      }
      const key = `${id}:${sessionId ?? "latest"}`;
      const ordered = [...events].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0));
      setOutputCache((prev) => ({ ...prev, [key]: ordered }));
      setOutputs(ordered);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    if (!token || !activeAgent) return;
    const key = `${activeAgent}:${activeSessionId ?? "latest"}`;
    const cached = outputCache[key];
    if (cached) {
      setOutputs(cached);
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
    if (!token || !activeAgent) return;
    loadAgentEvents(activeAgent, activeSessionId);
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
          if (activeSessionId && payload.session_id !== activeSessionId) {
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
          setOutputs((prev) =>
            [...prev, line].sort((a, b) => (a.seq ?? 0) - (b.seq ?? 0))
          );
          const key = `${payload.agent_id}:${payload.session_id ?? "latest"}`;
          setOutputCache((prev) => ({
            ...prev,
            [key]: [...(prev[key] ?? []), line].sort(
              (a, b) => (a.seq ?? 0) - (b.seq ?? 0)
            ),
          }));
        }
      } catch {
        // ignore
      }
    };
    ws.onclose = () => {
      if (wsRef.current === ws) {
        wsRef.current = null;
      }
    };
    return () => {
      if (wsRef.current === ws) {
        wsRef.current = null;
      }
      ws.close();
    };
  }, [token, activeAgent, activeSessionId]);

  useEffect(() => {
    const el = outputRef.current;
    if (!el) return;
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    if (distance < 120) {
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
      setAgentName("");
      setAgentWorkdir("");
      setAgentCommand("agenthub-codex-acp");
      setWorktreeMode("use_existing");
      setWorktreeRepo("");
      setWorktreeRef("");
      setCodeMode(false);
      setShowCreateAgent(false);
    } catch (err) {
      const hint = formatWorktreeError(err);
      if (hint) {
        setWorktreeError(hint);
      } else {
        setError(String(err));
      }
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
      if (hint) {
        setWorktreeError(hint);
      } else {
        setError(String(err));
      }
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

  const onSendInput = async () => {
    if (!input.trim()) return;
    const text = input.trim();
    setInput("");
    if (!token || !activeAgent) return;
    if (!activeSessionId) {
      setError("no active session");
      return;
    }
    setOutputs((prev) => [
      ...prev,
      {
        agent_id: activeAgent,
        session_id: activeSessionId,
        ts: Math.floor(Date.now() / 1000),
        stream: "acp",
        message: JSON.stringify({ type: "user_message", text }),
      },
    ]);
    const ws = wsRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "input", data: text }));
      return;
    }
    setError("websocket not connected");
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

      {error && <div className="error">{error}</div>}

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
        <section className="workspace">
          <div className="workspace-left">
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
                  <button
                    key={agent.id}
                    className={
                      activeAgent === agent.id ? "agent-row active" : "agent-row"
                    }
                  onClick={() => {
                    setActiveAgent(agent.id);
                    setActiveSessionId(agentSessions[agent.id] ?? null);
                  }}
                >
                    <div className="agent-row-head">
                      <span className="agent-name">{agent.name}</span>
                      <span className={`agent-status ${agent.status}`}>
                        {agent.status}
                      </span>
                    </div>
                    <div className="agent-row-meta">{agent.command}</div>
                  </button>
                ))}
              </div>
              <div className="agent-detail">
                {activeAgentRecord ? (
                  <>
                    <div className="agent-detail-head">
                      <h3>{activeAgentRecord.name}</h3>
                      <span
                        className={`agent-status ${activeAgentRecord.status}`}
                      >
                        {activeAgentRecord.status}
                      </span>
                    </div>
                    <div className="agent-detail-meta">
                      <div>
                        <span className="label">ID</span>
                        <span className="value">{activeAgentRecord.id}</span>
                      </div>
                      <div>
                        <span className="label">Command</span>
                        <span className="value">
                          {activeAgentRecord.command}
                        </span>
                      </div>
                      <div>
                        <span className="label">Workdir</span>
                        <span className="value">
                          {activeAgentRecord.workdir}
                        </span>
                      </div>
                    </div>
                    {auth.role === "root" && (
                      <label className="checkbox-row compact">
                        <input
                          type="checkbox"
                          checked={activeAgentRecord.code_mode}
                          onChange={(e) =>
                            onSetCodeMode(
                              activeAgentRecord.id,
                              e.target.checked
                            )
                          }
                        />
                        <span>Code mode</span>
                      </label>
                    )}
                    <div className="actions">
                      {activeAgentRecord.status !== "running" && (
                        <button
                          onClick={() => onStartAgent(activeAgentRecord.id)}
                        >
                          Start
                        </button>
                      )}
                      {activeAgentRecord.status === "running" && (
                        <button
                          onClick={() => onStopAgent(activeAgentRecord.id)}
                        >
                          Stop
                        </button>
                      )}
                      <button
                        onClick={() => onDeleteAgent(activeAgentRecord.id)}
                      >
                        Delete
                      </button>
                    </div>
                  </>
                ) : (
                  <div className="empty">Select an agent to view details.</div>
                )}
              </div>
            </div>
          </div>
          <div className="workspace-right">
            <div className="output-header">
              <h2>Output</h2>
              {activeAgentRecord && (
                <span className="output-subtitle">
                  {activeAgentRecord.name}
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
                        {acpView.runStatus?.status && (
                          <span
                            className={`acp-run ${acpView.runStatus.status}`}
                          >
                            {acpView.runStatus.status}
                          </span>
                        )}
                        {acpView.thinkingStartTs && (
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
                      <div className="acp-conversation">
                        {acpView.messages.map((msg, idx) => (
                          <div
                            key={idx}
                            className={`acp-bubble ${msg.kind}`}
                          >
                            <div
                              className="acp-text"
                              dangerouslySetInnerHTML={{
                                __html: renderMarkdown(msg.text),
                              }}
                            />
                          </div>
                        ))}
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
                        {acpView.plan.entries.map((entry, idx) => (
                          <div key={idx} className="acp-plan-item">
                            <div className="title">{entry.content}</div>
                            {entry.status && (
                              <div className="meta">{entry.status}</div>
                            )}
                          </div>
                        ))}
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
            ) : (
              <div className="empty">Select an agent to view output.</div>
            )}
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
                <label className="checkbox-row">
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

      {auth && (
        <div className="input docked">
          <textarea
            placeholder="Send input (Enter to send, Shift+Enter for newline)"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                onSendInput();
              }
            }}
            rows={2}
          />
          <button onClick={onSendInput}>Send</button>
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

      {props.error && <div className="error">{props.error}</div>}

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
        {error && <div className="error">{error}</div>}
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

function escapeHtml(input: string): string {
  return input
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function renderMarkdown(input: string): string {
  const blocks = input.split("```");
  let out = "";
  blocks.forEach((part, idx) => {
    if (idx % 2 === 1) {
      const safe = escapeHtml(part.replace(/^\n/, "").replace(/\n$/, ""));
      out += `<pre><code>${safe}</code></pre>`;
      return;
    }
    let safe = escapeHtml(part);
    safe = safe.replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      (_m, text, href) =>
        `<a href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">${text}</a>`
    );
    safe = safe.replace(/`([^`]+)`/g, "<code>$1</code>");
    safe = safe.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    safe = safe.replace(/\*([^*]+)\*/g, "<em>$1</em>");
    out += safe;
  });
  return out;
}
