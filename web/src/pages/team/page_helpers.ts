import { formatAgentModelLabel } from "../../agent_presets";
import type {
  AgentEvent,
  AgentRecord,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamTaskRecord,
} from "../../api";
import type { StatusTone } from "../../components/status_badge";
import type { TeamMemberAgentStatusSummary } from "./member_helpers";

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

export const DEFAULT_TEAM_THREAD_TITLE = "all";

export type TeamRuntimeStatusView = {
  status: "running" | "stopped" | "degraded";
  label: string;
  tone: StatusTone;
  online: number;
  total: number;
};

export function resolveTeamRuntimeStatus(
  summary: TeamMemberAgentStatusSummary | null
): TeamRuntimeStatusView {
  const online = summary?.active ?? 0;
  const total = summary?.total ?? 0;
  const missing = summary?.missing ?? 0;
  if (total === 0 || online === 0) {
    return {
      status: "stopped",
      label: "team stopped",
      tone: "inactive",
      online,
      total,
    };
  }
  if (online === total && missing === 0) {
    return {
      status: "running",
      label: "team running",
      tone: "active",
      online,
      total,
    };
  }
  return {
    status: "degraded",
    label: "team degraded",
    tone: "warning",
    online,
    total,
  };
}

export function upsertRun(list: TeamRunRecord[], nextRun: TeamRunRecord): TeamRunRecord[] {
  const withoutCurrent = list.filter((run) => run.id !== nextRun.id);
  return sortRuns([nextRun, ...withoutCurrent]);
}

export function upsertEventList(
  prev: TeamRunEventRecord[],
  next: TeamRunEventRecord[],
  mode: "replace" | "prepend"
): TeamRunEventRecord[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, TeamRunEventRecord>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

export function upsertAgentEventList(
  prev: AgentEvent[],
  next: AgentEvent[],
  mode: "replace" | "prepend"
): AgentEvent[] {
  const merged = mode === "replace" ? [...next] : [...next, ...prev];
  const byId = new Map<number, AgentEvent>();
  for (const event of merged) {
    byId.set(event.event_id, event);
  }
  return [...byId.values()].sort((a, b) => a.event_id - b.event_id);
}

export function buildAgentLabel(agent: AgentRecord): string {
  const model = formatAgentModelLabel(agent.command, agent.args) ?? "Unknown";
  return `${agent.name} · ${model} · ${agent.id.slice(0, 8)}`;
}

export function pickNextWorkerAgentId(
  agents: AgentRecord[],
  excludedAgentIds: Set<string>
): string {
  return agents.find((agent) => !excludedAgentIds.has(agent.id))?.id ?? "";
}

export function sortTasksByActivity(tasks: TeamTaskRecord[]): TeamTaskRecord[] {
  return [...tasks].sort((left, right) => {
    if (right.updated_at !== left.updated_at) {
      return right.updated_at - left.updated_at;
    }
    if (right.created_at !== left.created_at) {
      return right.created_at - left.created_at;
    }
    return right.id.localeCompare(left.id);
  });
}

export function isSharedThreadTask(task: TeamTaskRecord): boolean {
  if (task.title.trim().toLowerCase() === DEFAULT_TEAM_THREAD_TITLE) {
    return true;
  }
  if (!task.context || typeof task.context !== "object" || Array.isArray(task.context)) {
    return false;
  }
  return (task.context as { bootstrap_kind?: unknown }).bootstrap_kind === "shared_thread";
}

export function resolveTeamConversationTask(
  tasks: TeamTaskRecord[],
  teamId: string
): TeamTaskRecord | null {
  const teamTasks = sortTasksByActivity(tasks.filter((task) => task.team_id === teamId));
  return teamTasks.find(isSharedThreadTask) ?? null;
}

export function listTeamWorkspaceTasks(
  tasks: TeamTaskRecord[],
  teamId: string
): TeamTaskRecord[] {
  return sortTasksByActivity(
    tasks.filter((task) => task.team_id === teamId && !isSharedThreadTask(task))
  );
}

export function resolveSelectedTeamTask(
  tasks: TeamTaskRecord[],
  selectedTaskId: string,
  teamId: string
): TeamTaskRecord | null {
  const teamTasks = listTeamWorkspaceTasks(tasks, teamId);
  const selectedId = selectedTaskId.trim();
  if (selectedId) {
    const selected = teamTasks.find((task) => task.id === selectedId);
    if (selected) {
      return selected;
    }
  }
  return teamTasks[0] ?? null;
}

function parseMailboxPayload(payload: unknown): unknown {
  if (typeof payload !== "string") {
    return payload;
  }
  const trimmed = payload.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) {
    return payload;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return payload;
  }
}

export function resolveTaskMessageSeenByActors(
  mailboxMessages: TeamActorMessageRecord[],
  conversationId: string,
  memberIds: string[]
): Record<number, string[]> {
  const conversationKey = conversationId.trim();
  if (!conversationKey) {
    return {};
  }
  const memberSet = new Set(memberIds.map((memberId) => memberId.trim()).filter(Boolean));
  if (memberSet.size === 0) {
    return {};
  }
  const seenByMessageId = new Map<number, Set<string>>();
  for (const mailboxMessage of mailboxMessages) {
    if (mailboxMessage.status !== "delivered") {
      continue;
    }
    const toActorId = mailboxMessage.to_actor_id.trim();
    if (!memberSet.has(toActorId)) {
      continue;
    }
    const payload = parseMailboxPayload(mailboxMessage.payload);
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
      continue;
    }
    const taskConversationId = String(
      (payload as { task_conversation_id?: unknown }).task_conversation_id ?? ""
    ).trim();
    if (taskConversationId !== conversationKey) {
      continue;
    }
    const rawTaskMessageId = (payload as { task_message_id?: unknown }).task_message_id;
    const taskMessageId =
      typeof rawTaskMessageId === "number"
        ? rawTaskMessageId
        : typeof rawTaskMessageId === "string"
          ? Number.parseInt(rawTaskMessageId, 10)
          : Number.NaN;
    if (!Number.isFinite(taskMessageId)) {
      continue;
    }
    const seenActors = seenByMessageId.get(taskMessageId) ?? new Set<string>();
    seenActors.add(toActorId);
    seenByMessageId.set(taskMessageId, seenActors);
  }
  return Object.fromEntries(
    [...seenByMessageId.entries()].map(([messageId, actorIds]) => [
      messageId,
      [...actorIds].sort((left, right) => left.localeCompare(right)),
    ])
  );
}

export function formatTs(ts?: number | null): string {
  if (!ts) return "-";
  return new Date(ts * 1000).toLocaleString();
}

export function toPrettyJson(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
