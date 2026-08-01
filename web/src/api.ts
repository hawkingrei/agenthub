import {
  clearAuthAndRedirect,
  shouldRedirectOnAuthError,
} from "./auth_redirect";

export const AGENT_SOURCE_MANUAL = "manual";
export const AGENT_SOURCE_TEAM_FORGE = "team_forge";
export const AGENT_EVENT_PAGE_SIZE = 20;
export const AGENT_NOT_RUNNING_ERROR = "agent not running";

export function isAgentActiveStatus(status: string | null): boolean {
  return status === "running" || status === "starting";
}

export function isAgentSseTargetStatus(status: string | null): boolean {
  return status === "running" || status === "starting" || status === "idle";
}

export type AgentConfig = {
  name: string;
  workdir: string;
  command: string;
  args: string[];
  target_node_id?: string | null;
  source?: typeof AGENT_SOURCE_MANUAL | typeof AGENT_SOURCE_TEAM_FORGE;
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  codex_acp_default_mode?: string | null;
  runtime_model?: string | null;
  thinking_level?: string | null;
  agent_loop_enabled?: boolean;
  agent_loop_idle_seconds?: number | null;
  agent_loop_prompt?: string | null;
};

export type AgentRecord = {
  id: string;
  name: string;
  workdir: string;
  command: string;
  args: string[];
  target_node_id?: string | null;
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
  code_mode: boolean;
  codex_acp_default_mode?: string | null;
  runtime_model?: string | null;
  thinking_level?: string | null;
  agent_loop_enabled?: boolean;
  agent_loop_idle_seconds?: number | null;
  agent_loop_prompt?: string | null;
  status: string;
  created_at: number;
  updated_at: number;
};

export type AgentTimeTriggerRecord = {
  id: string;
  agent_id: string;
  kind: string;
  created_by_actor_id: string;
  message_text: string;
  fire_at: number;
  status: "scheduled" | "dispatching" | "fired" | "canceled";
  created_at: number;
  updated_at: number;
  fired_at: number | null;
  last_error: string | null;
};

export type AgentDiscoveryIdentityRecord = {
  agent_id: string;
  name: string;
  status: string;
};

export type AgentDiscoveryRuntimeRecord = {
  acp_provider?: string | null;
  code_mode: boolean;
  codex_acp_default_mode?: string | null;
  runtime_model?: string | null;
  thinking_level?: string | null;
  agent_loop_enabled?: boolean;
  agent_loop_idle_seconds?: number | null;
  target_node_id?: string | null;
  worktree_mode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktree_repo?: string | null;
  worktree_ref?: string | null;
};

export type AgentNodeConfig = {
  id: string;
  name: string;
  grpc_target: string;
  tls_server_name?: string | null;
  default_worktree_root?: string | null;
};

export type AgentNodeUpdate = {
  name: string;
  grpc_target: string;
  tls_server_name?: string | null;
  default_worktree_root?: string | null;
};

export type AgentNodeRecord = {
  id: string;
  name: string;
  grpc_target?: string | null;
  tls_server_name?: string | null;
  default_worktree_root?: string | null;
  last_seen_at?: number | null;
  is_main: boolean;
  created_at: number;
  updated_at: number;
};

export type AgentNodeJoinBootstrapInfo = {
  enabled: boolean;
  bootstrap_token?: string | null;
  grpc_listen_addr?: string | null;
  security_mode?: string | null;
  cert_dir?: string | null;
  issuer?: string | null;
  audience?: string | null;
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
  challenge_id?: string | null;
  options?: unknown;
  registration_options?: unknown;
  user_id?: string | null;
  token?: string | null;
  role?: string | null;
};

export type AuthFinishResponse = {
  user_id: string;
  token: string;
  role: string;
};

export type AuthStatusResponse = {
  root_initialized: boolean;
  passkey_enabled: boolean;
};

export type AdminSettingsResponse = {
  passkey_enabled: boolean;
};

export type AppLinkerPrincipal = {
  subject: string;
  principal_type: "human" | "agent";
  display_name: string;
  handle: string | null;
  avatar_url: string | null;
  server_id: string | null;
  server_slug: string | null;
  updated_at: number;
};

export type AppLinkerRecord = {
  linker_id: string;
  connector_id: string;
  display_name: string;
  status: "configured" | "connected" | string;
  api_origin: string;
  client_id: string;
  return_url: string;
  scopes: string[];
  client_secret_configured: boolean;
  token_configured: boolean;
  token_type: string | null;
  granted_scopes: string[];
  expires_at: number | null;
  principal: AppLinkerPrincipal | null;
  updated_at: number;
};

export type UpsertSlockLinkerRequest = {
  api_origin: string;
  client_id: string;
  client_secret?: string | null;
  return_url: string;
  scopes: string[];
};

export type SlockLinkAttemptResponse = {
  linker_id: string;
  state: string;
  expires_at: number;
  return_url: string;
};

export type JoinStartResponse = {
  challenge_id?: string | null;
  options?: unknown;
  user_id?: string | null;
  token?: string | null;
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
  last_login_at: number | null;
};

export type AuditRecord = {
  id: number;
  user_id: string | null;
  device_id: string | null;
  event: string;
  ip: string | null;
  user_agent: string | null;
  detail: string | null;
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

export type TeamPromptDefaultsRecord = {
  coordinator_prompt: string;
  worker_prompt: string;
};

export type TeamRunStatus =
  | "submitted"
  | "working"
  | "input_required"
  | "completed"
  | "failed"
  | "canceled";

export type TeamTaskStatus =
  | "open"
  | "in_progress"
  | "waiting"
  | "in_review"
  | "completed"
  | "canceled";

export type TeamTaskPriority = "critical" | "high" | "medium" | "low";

export type TeamTaskNoteKind = "comment" | "decision" | "result";

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

export type TeamChannelRecord = {
  team_id: string;
  channel_id: string;
  task_id: string;
  conversation_id: string;
  description?: string | null;
  created_by_actor_id: string;
  created_at: number;
  updated_at: number;
};

export type TeamTaskRecord = {
  id: string;
  team_id: string;
  title: string;
  status: TeamTaskStatus;
  priority?: TeamTaskPriority | null;
  created_by_actor_id: string;
  assigned_member_id?: string | null;
  context: unknown;
  created_at: number;
  updated_at: number;
};

export type CreateTeamTaskFromChannelMessagePayload = {
  title?: string | null;
  priority?: TeamTaskPriority | null;
  assigned_member_id?: string | null;
  created_by_actor_id?: string | null;
  context?: unknown;
};

export type TeamConversationRecord = {
  id: string;
  team_id: string;
  task_id: string;
  mode: "to_coordinator" | "to_member" | "group_chat";
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
  route: "to_coordinator" | "to_member" | "group_chat" | "team_thread_reply";
  payload: unknown;
  created_at: number;
};

export type TeamTaskNoteRecord = {
  message_id: number;
  conversation_id: string;
  task_id: string;
  from_actor_id: string;
  kind: TeamTaskNoteKind;
  text: string;
  created_at: number;
};

export type TeamTaskDetailResponse = {
  task: TeamTaskRecord;
  conversation: TeamConversationRecord;
  latest_run?: TeamRunRecord | null;
  notes?: TeamTaskNoteRecord[];
};

export type TeamThreadOpenRecord = {
  team_id: string;
  channel_id: string;
  task_id: string;
  conversation_id: string;
  root_message_id: number;
  thread_id: string;
};

export type TeamThreadReplyRecord = {
  thread: TeamThreadOpenRecord;
  message: TeamConversationMessageRecord;
};

export type TeamUploadRequest = {
  file_name: string;
  content_type: string;
  bytes_base64: string;
  expected_size_bytes?: number | null;
  expected_sha256?: string | null;
};

export type ObjectUploadRecord = {
  id: string;
  owner_scope: string;
  backend: string;
  object_key: string;
  original_filename: string;
  content_type: string;
  size_bytes: number;
  sha256: string;
  public_url?: string | null;
  created_by_actor_id: string;
  publish_state: string;
  created_at: number;
  published_at?: number | null;
  cleanup_after?: number | null;
};

export type TeamRunRecord = {
  id: string;
  team_id: string;
  context_id: string;
  status: TeamRunStatus;
  input: unknown;
  summary?: string | null;
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
  runtime_handle_id?: string | null;
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
  handling_disposition?:
    | "untriaged"
    | "ignored"
    | "watching"
    | "claimed"
    | "completed"
    | "released";
  handled_by_actor_id?: string | null;
  thread_topic_key?: string | null;
  thread_claim_status?: "claimed" | "released" | "completed" | null;
  thread_owner_actor_id?: string | null;
  thread_lease_expires_at?: number | null;
  linked_task_id?: string | null;
  linked_task_relation?: "spawned_task" | "related_task" | "evidence_for_task" | null;
  created_at: number;
  delivered_at?: number | null;
};

export type TeamReplyObligationRecord = {
  message_id: number;
  agent_actor_id: string;
  human_actor_id: string;
  source_surface: string;
  reply_target?: unknown | null;
  conversation_id?: string | null;
  thread_root_message_id?: number | null;
  text_excerpt?: string | null;
  created_at: number;
};

export type TeamMemberSnapshot = {
  member_id: string;
  role: string;
  model?: string | null;
  description?: string | null;
  prompt?: string | null;
  skills: string[];
  pending_inbox_count: number;
  reply_obligation_count?: number | null;
  status: string;
  latest_step?: TeamStepRecord | null;
  session_id?: string | null;
  session_status?: string | null;
};

export type TeamMailboxSnapshot = {
  pending: number;
  delivered: number;
  dead_letter: number;
  open_reply_obligation_count?: number | null;
  open_reply_obligations?: TeamReplyObligationRecord[];
  recent_messages: TeamActorMessageRecord[];
};

export type TeamRunSnapshotRecord = {
  run: TeamRunRecord;
  team: TeamDefinitionRecord;
  coordinator_member_id?: string | null;
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

export type TeamRuntimeMemberStatusResponse = {
  member_id: string;
  session_id: string;
  action: string;
};

export type TeamRuntimeControlResponse = {
  team_id: string;
  status: string;
  members: TeamRuntimeMemberStatusResponse[];
};

export type TeamRuntimeMemberRecord = {
  member_id: string;
  display_name: string;
  role: string;
  description?: string | null;
  agent_status?: string | null;
  session_id?: string | null;
  session_status?: string | null;
  card: {
    card_id: string;
    schema_version: string;
    description: string;
    capability_tags: string[];
  };
};

export type TeamRuntimeRecord = {
  team_id: string;
  team_name: string;
  status: "running" | "stopped" | "degraded";
  members: TeamRuntimeMemberRecord[];
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

export function stringifyApiError(err: unknown): string {
  return parseApiErrorMessage(err) ?? String(err);
}

export function getApiErrorStatus(err: unknown): number | null {
  if (!err || typeof err !== "object") {
    return null;
  }
  const { status } = err as { status?: unknown };
  return typeof status === "number" ? status : null;
}

function authHeaders(token: string | null): HeadersInit {
  if (!token) return {};
  return { Authorization: `Bearer ${token}` };
}

function buildApiHeaders(
  token: string | null,
  init?: RequestInit
): Headers {
  const headers = new Headers(init?.headers ?? {});
  const auth = authHeaders(token);
  for (const [key, value] of Object.entries(auth)) {
    headers.set(key, value);
  }
  if (init?.body !== undefined && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  return headers;
}

type ApiFetchInit = RequestInit & {
  networkRetry?: "auto" | "never" | "always";
};

type HttpStatusError = Error & {
  status?: number;
};

const NETWORK_RETRY_DELAYS_MS = [150, 350] as const;

function normalizeRequestMethod(init?: ApiFetchInit): string {
  return (init?.method ?? "GET").toUpperCase();
}

function shouldRetryNetworkFailure(init?: ApiFetchInit): boolean {
  const policy = init?.networkRetry ?? "auto";
  if (policy === "always") {
    return true;
  }
  if (policy === "never") {
    return false;
  }
  const method = normalizeRequestMethod(init);
  return method === "GET" || method === "HEAD" || method === "OPTIONS";
}

function isNetworkJitterError(err: unknown): boolean {
  if (!(err instanceof Error)) {
    return false;
  }
  if (err.name === "AbortError") {
    return false;
  }
  const message = err.message.trim().toLowerCase();
  return (
    message.includes("failed to fetch") ||
    message.includes("fetch failed") ||
    message.includes("networkerror") ||
    message.includes("network error") ||
    message.includes("load failed") ||
    message.includes("connection reset") ||
    message.includes("connection aborted") ||
    message.includes("connection refused") ||
    message.includes("network request failed")
  );
}

function shouldRetryNetworkJitter(
  err: unknown,
  init?: ApiFetchInit
): boolean {
  if (
    err instanceof Error &&
    typeof (err as HttpStatusError).status === "number"
  ) {
    return false;
  }
  if (!isNetworkJitterError(err) || !shouldRetryNetworkFailure(init)) {
    return false;
  }
  if (typeof navigator !== "undefined" && navigator.onLine === false) {
    return false;
  }
  return true;
}

function waitForBackoff(delayMs: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, delayMs);
  });
}

async function apiFetch<T>(
  path: string,
  token: string | null,
  init?: ApiFetchInit
): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      const { networkRetry: _networkRetry, ...fetchInit } = init ?? {};
      void _networkRetry;
      const res = await fetch(path, {
        ...fetchInit,
        headers: buildApiHeaders(token, init),
      });
      if (!res.ok) {
        const raw = await res.text();
        const msg = parseApiErrorText(raw);
        if (shouldRedirectOnAuthError(res.status, token, msg)) {
          clearAuthAndRedirect();
        }
        const error = new Error(msg || raw || res.statusText) as Error & {
          status?: number;
        };
        error.status = res.status;
        throw error;
      }
      return (await res.json()) as T;
    } catch (err) {
      if (
        shouldRetryNetworkJitter(err, init) &&
        attempt < NETWORK_RETRY_DELAYS_MS.length
      ) {
        await waitForBackoff(NETWORK_RETRY_DELAYS_MS[attempt]);
        continue;
      }
      throw err;
    }
  }
}

export const __testOnlyApiInternals = {
  apiFetch,
};

function encodePathSegment(value: string | number): string {
  return encodeURIComponent(String(value));
}

export function buildTeamRunContextSseUrl(
  origin: string,
  teamId: string,
  runId: string,
  token: string
): string {
  return `${origin}/sse/teams/${encodePathSegment(teamId)}/runs/${encodePathSegment(
    runId
  )}/context?token=${encodeURIComponent(token)}`;
}

export function buildTeamRuntimeSseUrl(
  origin: string,
  teamId: string,
  token: string
): string {
  return `${origin}/sse/teams/${encodePathSegment(
    teamId
  )}/runtime?token=${encodeURIComponent(token)}`;
}

export const api = {
  registerStart: (
    username: string,
    display_name: string,
    role?: string,
    password?: string,
    device_name?: string
  ) =>
    apiFetch<AuthStartResponse>("/api/auth/register/start", null, {
      method: "POST",
      body: JSON.stringify({ username, display_name, role, password, device_name }),
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
  loginRegisterFinish: (challenge_id: string, credential: unknown) =>
    apiFetch<AuthFinishResponse>("/api/auth/login/register_finish", null, {
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
  getAdminSettings: (token: string) =>
    apiFetch<AdminSettingsResponse>("/api/admin/settings", token),
  setPasskeyEnabled: (token: string, enabled: boolean) =>
    apiFetch<{ status: string }>("/api/admin/settings/passkey", token, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    }),
  listLinkers: (token: string) =>
    apiFetch<AppLinkerRecord[]>("/api/admin/linkers", token),
  upsertSlockLinker: (token: string, payload: UpsertSlockLinkerRequest) =>
    apiFetch<AppLinkerRecord>("/api/admin/linkers/slock", token, {
      method: "PUT",
      body: JSON.stringify(payload),
    }),
  createSlockLinkAttempt: (token: string) =>
    apiFetch<SlockLinkAttemptResponse>(
      "/api/admin/linkers/slock/link_attempts",
      token,
      { method: "POST" }
    ),
  exchangeSlockCode: (
    token: string,
    payload: { code?: string | null; callback_url?: string | null; state?: string | null }
  ) =>
    apiFetch<AppLinkerRecord>("/api/admin/linkers/slock/exchange", token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  createTeam: (
    token: string,
    payload: { name: string; description?: string; spec: unknown }
  ) =>
    apiFetch<TeamDefinitionRecord>("/api/teams", token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  getTeamPromptDefaults: (token: string) =>
    apiFetch<TeamPromptDefaultsRecord>("/api/teams/prompt_defaults", token),
  listTeams: (token: string) =>
    apiFetch<TeamDefinitionRecord[]>("/api/teams", token),
  getTeam: (token: string, id: string) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}`, token),
  updateTeamSpec: (
    token: string,
    id: string,
    payload: { spec: unknown; expected_updated_at: number }
  ) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}/spec`, token, {
      method: "PUT",
      body: JSON.stringify(payload),
    }),
  moveExistingAgentToTeam: (
    token: string,
    id: string,
    payload: { agent_id: string; spec: unknown; expected_updated_at: number }
  ) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}/members/move`, token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  getTeamRuntime: (token: string, id: string) =>
    apiFetch<TeamRuntimeRecord>(`/api/teams/${encodePathSegment(id)}/runtime`, token),
  startTeam: (token: string, id: string) =>
    apiFetch<TeamRuntimeControlResponse>(
      `/api/teams/${encodePathSegment(id)}/start`,
      token,
      { method: "POST" }
    ),
  stopTeam: (token: string, id: string) =>
    apiFetch<TeamRuntimeControlResponse>(
      `/api/teams/${encodePathSegment(id)}/stop`,
      token,
      { method: "POST" }
    ),
  forceTeamMemberNewSession: (token: string, teamId: string, memberId: string) =>
    apiFetch<TeamRuntimeControlResponse>(
      `/api/teams/${encodePathSegment(teamId)}/members/${encodePathSegment(memberId)}/force_new_session`,
      token,
      { method: "POST" }
    ),
  deleteTeam: (token: string, id: string) =>
    apiFetch<TeamDefinitionRecord>(`/api/teams/${encodePathSegment(id)}`, token, {
      method: "DELETE",
    }),
  getTeamSharedThread: (token: string, teamId: string) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/shared_thread`,
      token
    ),
  ensureTeamSharedThread: (token: string, teamId: string) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/shared_thread`,
      token,
      { method: "POST" }
    ),
  listTeamChannels: (token: string, teamId: string) =>
    apiFetch<TeamChannelRecord[]>(
      `/api/teams/${encodePathSegment(teamId)}/channels`,
      token
    ),
  createTeamChannel: (
    token: string,
    teamId: string,
    payload: { channel_id: string; description?: string | null }
  ) =>
    apiFetch<TeamChannelRecord>(
      `/api/teams/${encodePathSegment(teamId)}/channels`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  deleteTeamChannel: (token: string, teamId: string, channelId: string) =>
    apiFetch<TeamChannelRecord>(
      `/api/teams/${encodePathSegment(teamId)}/channels/${encodePathSegment(channelId)}`,
      token,
      {
        method: "DELETE",
      }
    ),
  listTeamTasks: (
    token: string,
    teamId: string,
    limit?: number,
    payload?: {
      include_shared_thread?: boolean;
      priority?: TeamTaskPriority | "all";
    }
  ) => {
    const params = new URLSearchParams();
    if (limit != null) params.set("limit", String(limit));
    if (payload?.include_shared_thread) {
      params.set("include_shared_thread", "true");
    }
    if (payload?.priority && payload.priority !== "all") {
      params.set("priority", payload.priority);
    }
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiFetch<TeamTaskRecord[]>(
      `/api/teams/${encodePathSegment(teamId)}/tasks${suffix}`,
      token
    );
  },
  createTeamTaskFromChannelMessage: (
    token: string,
    teamId: string,
    channelId: string,
    messageId: number,
    payload: CreateTeamTaskFromChannelMessagePayload
  ) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/channels/${encodePathSegment(channelId)}/messages/${messageId}/tasks`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  getTeamTask: (token: string, teamId: string, taskId: string) =>
    apiFetch<TeamTaskDetailResponse>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}`,
      token
    ),
  updateTeamTask: (
    token: string,
    teamId: string,
    taskId: string,
    payload: { status?: TeamTaskStatus; assigned_member_id?: string | null }
  ) =>
    apiFetch<TeamTaskRecord>(
      `/api/teams/${encodePathSegment(teamId)}/tasks/${encodePathSegment(taskId)}`,
      token,
      {
        method: "PATCH",
        body: JSON.stringify(payload),
      }
    ),
  sendTeamTaskMessage: (
    token: string,
    teamId: string,
    taskId: string,
    payload: {
      from_actor_id?: string;
      to_actor_id?: string;
      route?: "to_coordinator" | "to_member" | "group_chat";
      payload: unknown;
      idempotency_key?: string;
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
  replyTeamThread: (
    token: string,
    teamId: string,
    channelId: string,
    rootMessageId: number,
    payload: { text: string; mention_actor_ids?: string[] }
  ) =>
    apiFetch<TeamThreadReplyRecord>(
      `/api/teams/${encodePathSegment(teamId)}/channels/${encodePathSegment(channelId)}/threads/${rootMessageId}/replies`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
  uploadTeamImage: (token: string, teamId: string, payload: TeamUploadRequest) =>
    apiFetch<ObjectUploadRecord>(
      `/api/teams/${encodePathSegment(teamId)}/images`,
      token,
      {
        method: "POST",
        body: JSON.stringify(payload),
      }
    ),
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
    payload: { runtime_handle_id?: string }
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
  triageTeamRunMessage: (
    token: string,
    runId: string,
    messageId: number,
    payload: {
      actor_id: string;
      disposition: "ignored" | "watching" | "claimed" | "completed" | "released";
    }
  ) =>
    apiFetch<TeamActorMessageRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/messages/${encodePathSegment(messageId)}/triage`,
      token,
      { method: "POST", body: JSON.stringify(payload) }
    ),
  escalateTeamRunMessage: (
    token: string,
    runId: string,
    messageId: number,
    actorId: string
  ) =>
    apiFetch<TeamActorMessageRecord>(
      `/api/teams/runs/${encodePathSegment(runId)}/messages/${encodePathSegment(messageId)}/escalate`,
      token,
      { method: "POST", body: JSON.stringify({ actor_id: actorId }) }
    ),
  listAgents: (token: string) => apiFetch<AgentRecord[]>("/api/agents", token),
  listAgentNodes: (token: string) =>
    apiFetch<AgentNodeRecord[]>("/api/agent_nodes", token),
  getAgentNodeJoinBootstrap: (token: string) =>
    apiFetch<AgentNodeJoinBootstrapInfo>("/api/agent_nodes/bootstrap", token),
  createAgentNode: (token: string, payload: AgentNodeConfig) =>
    apiFetch<AgentNodeRecord>("/api/agent_nodes", token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  updateAgentNode: (token: string, id: string, payload: AgentNodeUpdate) =>
    apiFetch<AgentNodeRecord>(`/api/agent_nodes/${encodePathSegment(id)}`, token, {
      method: "PATCH",
      body: JSON.stringify(payload),
    }),
  deleteAgentNode: (token: string, id: string) =>
    apiFetch<{ status: string }>(`/api/agent_nodes/${encodePathSegment(id)}`, token, {
      method: "DELETE",
    }),
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
  getAgentEvent: (token: string, id: string, eventId: number) =>
    apiFetch<AgentEvent>(
      `/api/agents/${encodePathSegment(id)}/events/${encodePathSegment(String(eventId))}`,
      token
    ),
  setAgentCodeMode: (token: string, id: string, code_mode: boolean) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/code_mode`, token, {
      method: "POST",
      body: JSON.stringify({ code_mode }),
    }),
  setAgentCodexAcpDefaultMode: (
    token: string,
    id: string,
    mode_id: string | null
  ) =>
    apiFetch<{ status: string }>(
      `/api/agents/${encodePathSegment(id)}/codex_acp_default_mode`,
      token,
      {
        method: "POST",
        body: JSON.stringify({ mode_id }),
      }
    ),
  setAgentRuntimeProfile: (
    token: string,
    id: string,
    payload: { runtime_model?: string | null; thinking_level?: string | null }
  ) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/runtime_profile`, token, {
      method: "POST",
      body: JSON.stringify(payload),
    }),
  setAgentLoop: (
    token: string,
    id: string,
    payload: {
      enabled: boolean;
      idle_seconds?: number | null;
      prompt?: string | null;
    }
  ) =>
    apiFetch<{ status: string }>(`/api/agents/${encodePathSegment(id)}/agent_loop`, token, {
      method: "POST",
      body: JSON.stringify(payload),
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
  listAgentTimeTriggers: (
    token: string,
    id: string,
    limit?: number
  ) =>
    apiFetch<AgentTimeTriggerRecord[]>(
      `/api/agents/${encodePathSegment(id)}/triggers${limit != null ? `?limit=${limit}` : ""}`,
      token
    ),
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

export function getTeamStepRuntimeHandleId(
  step?: Pick<TeamStepRecord, "runtime_handle_id" | "remote_task_id"> | null
): string | null {
  const runtimeHandleId = step?.runtime_handle_id ?? step?.remote_task_id ?? null;
  if (!runtimeHandleId) {
    return null;
  }
  const trimmed = runtimeHandleId.trim();
  return trimmed.length > 0 ? trimmed : null;
}
