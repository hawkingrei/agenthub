import {
  AcpPermissionRecord,
  AgentEvent,
  AgentNodeRecord,
  AgentRecord,
} from "./api";
import { buildAcpView, EMPTY_ACP_VIEW, type AcpView } from "./acp";
import { compareEventOrder } from "./seq_order";
import { getLocalStorageItemSafe, setLocalStorageItemSafe } from "./storage/safe_storage";
import { AuthState } from "./types";

const PERMISSION_JUMP_MAX_ATTEMPTS = 24;
const AGENTS_PANEL_WIDTH_STORAGE_KEY = "agenthub_agents_panel_width";
const AGENTS_PANEL_DEFAULT_WIDTH = 288;
const AGENTS_PANEL_MIN_WIDTH = 256;
const AGENTS_PANEL_MAX_WIDTH = 352;
const AGENTS_PANEL_MIN_RIGHT_WIDTH = 760;
const AGENTS_WORKSPACE_SPLITTER_WIDTH = 12;

export type PendingPermissionJumpState = {
  toolCallId: string;
  sessionId: string | null;
  attempts: number;
};

export type PermissionJumpDecision = "idle" | "wait" | "attempt" | "clear";

export function resolveDefaultActiveAgentId(agents: AgentRecord[]): string | null {
  return agents.find((agent) => agent.status === "running" || agent.status === "starting")?.id ?? null;
}

export function resolveActiveAcpView(
  activeAgent: string | null,
  acpOutputs: AgentEvent[]
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
        .filter((agent) => agent.status === "running" || agent.status === "starting")
        .map((agent) => agent.id)
    )
  ).sort();
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

function isSameAgentNodeRecord(a: AgentNodeRecord, b: AgentNodeRecord): boolean {
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

export function isSamePermissionList(
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

export function isSamePendingPermissionCountMap(
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
