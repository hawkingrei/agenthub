import {
  clearAuthAndRedirect,
  shouldRedirectOnAuthError,
} from "./auth_redirect";

export type AgentConfig = {
  name: string;
  workdir: string;
  command: string;
  args: string[];
  source?: "manual" | "team_forge";
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

export type AgentDiscoveryIdentityRecord = {
  agent_id: string;
  name: string;
  status: string;
};

export type AgentDiscoveryRuntimeRecord = {
  acp_provider?: string | null;
  code_mode: boolean;
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
};

export type AgentDiscoveryCardRecord = {
  card_id: string;
  schema_version: string;
  description: string;
  identity: AgentDiscoveryIdentityRecord;
  runtime: AgentDiscoveryRuntimeRecord;
  capability_tags: string[];
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

export type RuntimeDefaults = {
  default_worktree_root: string;
};

export type TeamRunStatus =
  | "submitted"
  | "working"
  | "input_required"
  | "completed"
  | "failed"
  | "canceled";

export type TeamTaskStatus = "open" | "in_progress" | "completed" | "canceled";

export type TeamStepStatus =
  | "submitted"
  | "working"
  | "input_required"
  | "completed"
  | "failed"
  | "canceled";

export type TeamActorMessageTransport = "local" | "remote";

export type TeamActorMessageStatus = "pending" | "delivered" | "dead_letter";
export type TeamActorIdentityKind = "agent" | "human";

export type TeamDefinitionRecord = {
  id: string;
  name: string;
  description?: string | null;
  spec: unknown;
  created_at: number;
  updated_at: number;
};

export type TeamTaskRecord = {
  id: string;
  team_id: string;
  title: string;
  status: TeamTaskStatus;
  created_by_actor_id: string;
  context: unknown;
  created_at: number;
  updated_at: number;
};

export type TeamConversationRecord = {
  id: string;
  team_id: string;
  task_id: string;
  mode: "to_leader" | "to_member" | "group_chat";
  topic?: string | null;
  created_at: number;
  updated_at: number;
};

export type TeamConversationMessageRecord = {
  message_id: number;
  conversation_id: string;
  task_id: string;
  from_actor_id: string;
  to_actor_id?: string | null;
  route: "to_leader" | "to_member" | "group_chat";
  payload: unknown;
  created_at: number;
};

export type TeamTaskDetailResponse = {
  task: TeamTaskRecord;
  conversation: TeamConversationRecord;
};

export type TeamRunRecord = {
  id: string;
  team_id: string;
  context_id: string;
  status: TeamRunStatus;
  input: unknown;
  created_at: number;
  started_at?: number | null;
  ended_at?: number | null;
};

export type TeamRunEventRecord = {
  event_id: number;
  run_id: string;
  step_id?: string | null;
  event_type: string;
  ts: number;
  payload: unknown;
};

export type TeamStepRecord = {
  id: string;
  run_id: string;
  step_key: string;
  member_id: string;
  remote_task_id?: string | null;
  status: TeamStepStatus;
  attempt: number;
  depends_on: string[];
  input?: unknown;
  output?: unknown;
  error_text?: string | null;
  started_at?: number | null;
  ended_at?: number | null;
};

export type TeamActorMessageRecord = {
  message_id: number;
  run_id: string;
  from_actor_id: string;
  from_peer_id: string;
  from_actor_kind: TeamActorIdentityKind;
  to_actor_id: string;
  to_peer_id: string;
  to_actor_kind: TeamActorIdentityKind;
  channel: string;
  transport: TeamActorMessageTransport;
  route?: unknown;
  payload: unknown;
  status: TeamActorMessageStatus;
  created_at: number;
  delivered_at?: number | null;
};

export type TeamMemberSnapshot = {
  member_id: string;
  role: string;
  model?: string | null;
  description?: string | null;
  prompt?: string | null;
  skills: string[];
  pending_inbox_count: number;
  status: string;
  latest_step?: TeamStepRecord | null;
  session_status?: string | null;
};

export type TeamMailboxSnapshot = {
  pending: number;
  delivered: number;
  dead_letter: number;
  recent_messages: TeamActorMessageRecord[];
};

export type TeamRunSnapshotRecord = {
  run: TeamRunRecord;
  team: TeamDefinitionRecord;
  leader_member_id?: string | null;
  members: TeamMemberSnapshot[];
  steps: TeamStepRecord[];
  latest_events: TeamRunEventRecord[];
  mailbox: TeamMailboxSnapshot;
};

export type TeamCompiledStepTemplateRecord = {
  step_key: string;
  member_id: string;
  role: string;
  depends_on: string[];
};

export type TeamCompiledRoleAssignmentRecord = {
  member_id: string;
  role: string;
  step_keys: string[];
};

export type TeamTaskCompiledPlanRecord = {
  task_list: string[];
  acceptance_criteria: string[];
  deadline?: string | null;
  step_template: TeamCompiledStepTemplateRecord[];
  role_assignments: TeamCompiledRoleAssignmentRecord[];
  source_message_id?: number | null;
};

export type TeamRunPayloadPreviewRecord = {
  context_id: string;
  input: unknown;
};

export type TeamTaskRunCompilePreviewRecord = {
  task_id: string;
  conversation_id: string;
  run_payload: TeamRunPayloadPreviewRecord;
  plan: TeamTaskCompiledPlanRecord;
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

export function parseApiErrorMessage(err: unknown): string | null {
  if (!err) return null;
  if (typeof err === "string") {
    return parseApiErrorText(err);
  }
  if (err instanceof Error) {
    const raw = err.message ?? "";
    if (!raw) return null;
    return parseApiErrorText(raw) ?? raw;
  }
  if (typeof err === "object") {
    const value = err as { error?: unknown; message?: unknown };
    if (typeof value.error === "string" && value.error.trim().length > 0) {
      return value.error;
    }
    if (typeof value.message === "string" && value.message.trim().length > 0) {
      return parseApiErrorText(value.message) ?? value.message;
    }
  }
  return null;
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
    throw new Error(msg || raw || res.statusText);
  }
  return (await res.json()) as T;
}

function encodePathSegment(value: string | number): string {
  return encodeURIComponent(String(value));
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
  getRuntimeDefaults: (token: string) =>
    apiFetch<RuntimeDefaults>("/api/settings/defaults", token),
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
    apiFetch<{ status: string }>(
      `/api/admin/devices/${encodePathSegment(id)}/revoke`,
      token,
      { method: "POST" }
    ),
  listAudits: (token: string, limit = 100) =>
    apiFetch<AuditRecord[]>(`/api/admin/audits?limit=${limit}`, token),
  joinStartAdmin: (token: string) =>
    apiFetch<{ token: string; pin: string; expires_at: number }>(
      "/api/admin/join/start",
      token,
      { method: "POST" }
    ),
  createTeam: (
    token: string,
    payload: { name: string; description?: string; spec: unknown }
  ) =>
    apiFetch<TeamDefinitionRecord>("/api/teams", token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  listTeams: (token: string) =>
    apiFetch<TeamDefinitionRecord[]>("/api/teams", token),
  getTeam: (token: string, id: string) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}`, token),
  deleteTeam: (token: string, id: string) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}`, token, {
      method: "DELETE",
    }),
  createTeamTask: (
    token: string,
    teamId: string,
    payload: {
      title: string;
      created_by_actor_id?: string;
      context?: unknown;
      conversation_mode?: "to_leader" | "to_member" | "group_chat";
      topic?: string;
    }
  ) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/tasks`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  listTeamTasks: (token: string, teamId: string, limit?: number) => {
    const params = new URLSearchParams();
    if (limit != null) params.set("limit", String(limit));
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiFetch<TeamTaskRecord[]>(
      `/api/teams/${encodePathSegment(teamId)}/tasks${suffix}`,
      token
    );
  },
  getTeamTask: (token: string, teamId: string, taskId: string) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}`,
      token
    ),
  sendTeamTaskMessage: (
    token: string,
    teamId: string,
    taskId: string,
    payload: {
      from_actor_id?: string;
      to_actor_id?: string;
      route?: "to_leader" | "to_member" | "group_chat";
      payload: unknown;
    }
  ) =>
    apiFetch<TeamConversationMessageRecord>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}/messages`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  listTeamTaskMessages: (
    token: string,
    teamId: string,
    taskId: string,
    payload?: { limit?: number; before_id?: number }
  ) => {
    const params = new URLSearchParams();
    if (payload?.limit != null) params.set("limit", String(payload.limit));
    if (payload?.before_id != null) params.set("before_id", String(payload.before_id));
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiFetch<TeamConversationMessageRecord[]>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}/messages${suffix}`,
      token
    );
  },
  compileTeamTaskRunPreview: (
    token: string,
    teamId: string,
    taskId: string,
    payload: { context_id?: string }
  ) =>
    apiFetch<TeamTaskRunCompilePreviewRecord>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}/compile_run_preview`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  listTeamRuns: (
    token: string,
    teamId: string,
    payload?: {
      limit?: number;
      status?: TeamRunStatus;
      before_created_at?: number;
    }
  ) => {
    const params = new URLSearchParams();
    if (payload?.limit != null) params.set("limit", String(payload.limit));
    if (payload?.status) params.set("status", payload.status);
    if (payload?.before_created_at != null) {
      params.set("before_created_at", String(payload.before_created_at));
    }
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiFetch<TeamRunRecord[]>(
      `/api/teams/${encodePathSegment(teamId)}/runs${suffix}`,
      token
    );
  },
  createTeamRun: (
    token: string,
    teamId: string,
    payload: { context_id?: string; input?: unknown }
  ) =>
    apiFetch<TeamRunRecord>(`/api/teams/${encodePathSegment(teamId)}/runs`, token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  getTeamRun: (token: string, runId: string) =>
    apiFetch<TeamRunRecord>(`/api/teams/runs/${encodePathSegment(runId)}`, token),
  getTeamRunSnapshot: (
    token: string,
    runId: string,
    payload?: { event_limit?: number; message_limit?: number }
  ) => {
    const params = new URLSearchParams();
    if (payload?.event_limit != null) {
      params.set("event_limit", String(payload.event_limit));
    }
    if (payload?.message_limit != null) {
      params.set("message_limit", String(payload.message_limit));
    }
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiFetch<TeamRunSnapshotRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/snapshot${suffix}`,
      token
    );
  },
  cancelTeamRun: (token: string, runId: string) =>
    apiFetch<TeamRunRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/cancel`,
      token,
      { method: "POST" }
    ),
  resumeTeamRun: (token: string, runId: string) =>
    apiFetch<TeamRunRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/resume`,
      token,
      { method: "POST" }
    ),
  restartTeamRun: (token: string, runId: string) =>
    apiFetch<TeamRunRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/restart`,
      token,
      { method: "POST" }
    ),
  listTeamRunEvents: (
    token: string,
    runId: string,
    limit = 200,
    beforeId?: number
  ) => {
    const params = new URLSearchParams({ limit: String(limit) });
    if (beforeId != null) params.set("before_id", String(beforeId));
    return apiFetch<TeamRunEventRecord[]>(
      `/api/teams/runs/${encodePathSegment(runId)}/events?${params.toString()}`,
      token
    );
  },
  listTeamRunSteps: (token: string, runId: string) =>
    apiFetch<TeamStepRecord[]>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps`,
      token
    ),
  submitTeamRunStep: (
    token: string,
    runId: string,
    payload: {
      step_key: string;
      member_id: string;
      depends_on?: string[];
      input?: unknown;
    }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  startTeamRunStep: (
    token: string,
    runId: string,
    stepId: string,
    payload: { remote_task_id?: string }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps/${encodePathSegment(stepId)}/start`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  completeTeamRunStep: (
    token: string,
    runId: string,
    stepId: string,
    payload: { output?: unknown }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps/${encodePathSegment(stepId)}/complete`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  failTeamRunStep: (
    token: string,
    runId: string,
    stepId: string,
    payload: { error_text: string }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps/${encodePathSegment(stepId)}/fail`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  setTeamRunStepInputRequired: (
    token: string,
    runId: string,
    stepId: string,
    payload: { reason?: string; input?: unknown }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps/${encodePathSegment(stepId)}/input_required`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  resumeTeamRunStep: (
    token: string,
    runId: string,
    stepId: string,
    payload: { input?: unknown }
  ) =>
    apiFetch<TeamStepRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/steps/${encodePathSegment(stepId)}/resume`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  sendTeamRunMessage: (
    token: string,
    runId: string,
    payload: {
      from_actor_id: string;
      from_peer_id?: string;
      to_actor_id: string;
      to_peer_id?: string;
      channel?: string;
      transport?: TeamActorMessageTransport;
      route?: unknown;
      payload: unknown;
      idempotency_key?: string;
    }
  ) =>
    apiFetch<TeamActorMessageRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/messages/send`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  listTeamRunInbox: (
    token: string,
    runId: string,
    payload: {
      actor_id: string;
      limit?: number;
      after_id?: number;
      include_delivered?: boolean;
    }
  ) => {
    const params = new URLSearchParams({ actor_id: payload.actor_id });
    if (payload.limit != null) params.set("limit", String(payload.limit));
    if (payload.after_id != null) params.set("after_id", String(payload.after_id));
    if (payload.include_delivered != null) {
      params.set(
        "include_delivered",
        payload.include_delivered ? "true" : "false"
      );
    }
    return apiFetch<TeamActorMessageRecord[]>(
      `/api/teams/runs/${encodePathSegment(runId)}/messages/inbox?${params.toString()}`,
      token
    );
  },
  ackTeamRunMessage: (
    token: string,
    runId: string,
    messageId: number,
    actorId: string
  ) =>
    apiFetch<TeamActorMessageRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/messages/${encodePathSegment(messageId)}/ack`,
      token,
      { method: "POST", body: JSON.stringify({ actor_id: actorId }) }
    ),
  listAgents: (token: string) => apiFetch<AgentRecord[]>("/api/agents", token),
  getAgent: (token: string, id: string) =>
    apiFetch<AgentRecord>(`/api/agents/${encodePathSegment(id)}`, token),
  getAgentDiscoveryCard: (token: string, id: string) =>
    apiFetch<AgentDiscoveryCardRecord>(
      `/api/agents/${encodePathSegment(id)}/.well-known/agent-card`,
      token
    ),
  sendInput: (
    token: string,
    id: string,
    input: string,
    message_id?: string,
    session_id?: string
  ) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/input`, token, {
      method: "POST",
      body: JSON.stringify({ input, message_id, session_id }),
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
      `/api/agents/${encodePathSegment(id)}/events?${params.toString()}`,
      token
    );
  },
  setAgentCodeMode: (token: string, id: string, code_mode: boolean) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/code_mode`, token, {
      method: "POST",
      body: JSON.stringify({ code_mode }),
    }),
  clearAcpSession: (token: string, id: string, provider?: string) =>
    apiFetch<{ status: string }>(
      `/api/agents/${encodePathSegment(id)}/acp/session/clear`,
      token,
      { method: "POST", body: JSON.stringify(provider ? { provider } : {}) }
    ),
  setAcpMode: (token: string, id: string, mode_id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/acp/mode`, token, {
      method: "POST",
      body: JSON.stringify({ mode_id }),
    }),
  setAcpModel: (token: string, id: string, model_id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/acp/model`, token, {
      method: "POST",
      body: JSON.stringify({ model_id }),
    }),
  setAcpConfig: (
    token: string,
    id: string,
    config_id: string,
    value: string
  ) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/acp/config`, token, {
      method: "POST",
      body: JSON.stringify({ config_id, value }),
    }),
  cancelAcp: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/acp/cancel`, token, {
      method: "POST",
    }),
  startAgent: (token: string, id: string) =>
    apiFetch<{ session_id: string }>(`/api/agents/${encodePathSegment(id)}/start`, token, {
      method: "POST",
    }),
  stopAgent: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/stop`, token, {
      method: "POST",
    }),
  deleteAgent: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}`, token, {
      method: "DELETE",
    }),
  listAcpPermissions: (token: string, id: string, status?: string) => {
    const query = status ? `?status=${encodeURIComponent(status)}` : "";
    return apiFetch<AcpPermissionRecord[]>(
      `/api/agents/${encodePathSegment(id)}/permissions${query}`,
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
      `/api/agents/${encodePathSegment(agentId)}/permissions/${encodePathSegment(permissionId)}/respond`,
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
