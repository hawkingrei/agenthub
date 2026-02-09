import {
  clearAuthAndRedirect,
  shouldRedirectOnAuthError,
} from "./auth_redirect";

export type AgentConfig = {
  name: string;
  workdir: string;
  command: string;
  args: string[];
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
};

export type AgentRecord = {
  id: string;
  name: string;
  workdir: string;
  command: string;
  args: string[];
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  status: string;
  created_at: number;
  updated_at: number;
};

export type AgentEvent = {
  event_id: number;
  agent_id: string;
  session_id: string;
  seq: string;
  ts: number;
  stream: "stdout" | "stderr" | "system" | "acp";
  message: string;
};

export type AcpPermissionOption = {
  option_id: string;
  name: string;
  kind: string;
};

export type AcpPermissionRecord = {
  id: string;
  agent_id: string;
  session_id: string;
  acp_session_id?: string | null;
  tool_call_id?: string | null;
  options: AcpPermissionOption[];
  tool_call?: unknown;
  status: string;
  selected_option_id?: string | null;
  created_at: number;
  responded_at?: number | null;
};

export type AuthStartResponse = {
  challenge_id: string;
  options: unknown;
};

export type AuthFinishResponse = {
  user_id: string;
  token: string;
  role: string;
};

export type AuthStatusResponse = {
  root_initialized: boolean;
};

export type JoinStartResponse = {
  challenge_id: string;
  options: unknown;
};

export type JoinFinishResponse = {
  user_id: string;
  token: string;
};

export type SafePath = {
  path: string;
  created_at: number;
};

export type DeviceRecord = {
  id: string;
  user_id: string;
  name: string;
  user_agent: string;
  status: string;
  created_at: number;
  last_login_at?: number;
};

export type AuditRecord = {
  id: number;
  user_id?: string;
  device_id?: string;
  event: string;
  ip?: string;
  user_agent?: string;
  detail?: string;
  ts: number;
};

export type VapidInfo = {
  public_key: string;
  subject: string;
  keys_path: string;
};

export type VapidRotateResponse = {
  public_key: string;
};

function parseApiErrorText(raw: string): string | null {
  if (!raw) return null;
  if (!raw.trim().startsWith("{")) return raw;
  try {
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed.error === "string") {
      return parsed.error;
    }
  } catch {
    return raw;
  }
  return raw;
}

function authHeaders(token: string | null): HeadersInit {
  if (!token) return {};
  return { Authorization: `Bearer ${token}` };
}

async function apiFetch<T>(
  path: string,
  token: string | null,
  init?: RequestInit
): Promise<T> {
  const res = await fetch(path, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...authHeaders(token),
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const raw = await res.text();
    const msg = parseApiErrorText(raw);
    if (shouldRedirectOnAuthError(res.status, token, msg)) {
      clearAuthAndRedirect();
    }
    throw new Error(raw || res.statusText);
  }
  return (await res.json()) as T;
}

export const api = {
  registerStart: (
    username: string,
    display_name: string,
    role?: string,
    password?: string
  ) =>
    apiFetch<AuthStartResponse>("/api/auth/register/start", null, {
      method: "POST",
      body: JSON.stringify({ username, display_name, role, password }),
    }),
  registerFinish: (challenge_id: string, credential: unknown) =>
    apiFetch<AuthFinishResponse>("/api/auth/register/finish", null, {
      method: "POST",
      body: JSON.stringify({ challenge_id, credential }),
    }),
  loginStart: (username: string, password: string) =>
    apiFetch<AuthStartResponse>("/api/auth/login/start", null, {
      method: "POST",
      body: JSON.stringify({ username, password }),
    }),
  loginFinish: (challenge_id: string, credential: unknown) =>
    apiFetch<AuthFinishResponse>("/api/auth/login/finish", null, {
      method: "POST",
      body: JSON.stringify({ challenge_id, credential }),
    }),
  authStatus: () => apiFetch<AuthStatusResponse>("/api/auth/status", null),
  joinStart: (payload: {
    token: string;
    pin: string;
    username: string;
    display_name: string;
    password: string;
    device_name: string;
  }) =>
    apiFetch<JoinStartResponse>("/api/join/start", null, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  joinFinish: (challenge_id: string, credential: unknown) =>
    apiFetch<JoinFinishResponse>("/api/join/finish", null, {
      method: "POST",
      body: JSON.stringify({ challenge_id, credential }),
    }),
  listSafePaths: (token: string) =>
    apiFetch<SafePath[]>("/api/admin/safe_paths", token),
  addSafePath: (token: string, path: string) =>
    apiFetch<{ status: string }>("/api/admin/safe_paths", token, {
      method: "POST",
      body: JSON.stringify({ path }),
    }),
  deleteSafePath: (token: string, path: string) =>
    apiFetch<{ status: string }>("/api/admin/safe_paths", token, {
      method: "DELETE",
      body: JSON.stringify({ path }),
    }),
  listDevices: (token: string) =>
    apiFetch<DeviceRecord[]>("/api/admin/devices", token),
  revokeDevice: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/admin/devices/${id}/revoke`, token, {
      method: "POST",
    }),
  listAudits: (token: string, limit = 100) =>
    apiFetch<AuditRecord[]>(`/api/admin/audits?limit=${limit}`, token),
  joinStartAdmin: (token: string) =>
    apiFetch<{ token: string; pin: string; expires_at: number }>(
      "/api/admin/join/start",
      token,
      { method: "POST" }
    ),
  listAgents: (token: string) => apiFetch<AgentRecord[]>("/api/agents", token),
  sendInput: (token: string, id: string, input: string, message_id?: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/input`, token, {
      method: "POST",
      body: JSON.stringify({ input, message_id }),
    }),
  createAgent: (token: string, payload: AgentConfig) =>
    apiFetch<AgentRecord>("/api/agents", token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  listAgentEvents: (
    token: string,
    id: string,
    limit = 500,
    sessionId?: string,
    beforeId?: number | null
  ) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (sessionId) params.set("session_id", sessionId);
    if (beforeId != null) params.set("before_id", String(beforeId));
    return apiFetch<AgentEvent[]>(
      `/api/agents/${id}/events?${params.toString()}`,
      token
    );
  },
  setAgentCodeMode: (token: string, id: string, code_mode: boolean) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/code_mode`, token, {
      method: "POST",
      body: JSON.stringify({ code_mode }),
    }),
  clearAcpSession: (token: string, id: string, provider = "codex") =>
    apiFetch<{ status: string }>(`/api/agents/${id}/acp/session/clear`, token, {
      method: "POST",
      body: JSON.stringify({ provider }),
    }),
  setAcpMode: (token: string, id: string, mode_id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/acp/mode`, token, {
      method: "POST",
      body: JSON.stringify({ mode_id }),
    }),
  setAcpModel: (token: string, id: string, model_id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/acp/model`, token, {
      method: "POST",
      body: JSON.stringify({ model_id }),
    }),
  setAcpConfig: (
    token: string,
    id: string,
    config_id: string,
    value: string
  ) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/acp/config`, token, {
      method: "POST",
      body: JSON.stringify({ config_id, value }),
    }),
  cancelAcp: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/acp/cancel`, token, {
      method: "POST",
    }),
  startAgent: (token: string, id: string) =>
    apiFetch<{ session_id: string }>(`/api/agents/${id}/start`, token, {
      method: "POST",
    }),
  stopAgent: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}/stop`, token, {
      method: "POST",
    }),
  deleteAgent: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${id}`, token, {
      method: "DELETE",
    }),
  listAcpPermissions: (token: string, id: string, status?: string) => {
    const query = status ? `?status=${encodeURIComponent(status)}` : "";
    return apiFetch<AcpPermissionRecord[]>(
      `/api/agents/${id}/permissions${query}`,
      token
    );
  },
  respondAcpPermission: (
    token: string,
    agentId: string,
    permissionId: string,
    payload: { option_id?: string | null; outcome?: string }
  ) =>
    apiFetch<{ status: string }>(
      `/api/agents/${agentId}/permissions/${permissionId}/respond`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  getVapidPublicKey: () =>
    apiFetch<{ public_key: string }>("/api/push/vapid_public", null),
  getVapidInfo: (token: string) =>
    apiFetch<VapidInfo>("/api/push/vapid_info", token),
  rotateVapid: (token: string) =>
    apiFetch<VapidRotateResponse>("/api/push/vapid_rotate", token, {
      method: "POST",
    }),
  subscribePush: (token: string, sub: unknown) =>
    apiFetch<{ status: string }>("/api/push/subscribe", token, {
      method: "POST",
      body: JSON.stringify(sub),
    }),
};
