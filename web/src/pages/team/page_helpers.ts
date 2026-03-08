import { formatAgentModelLabel } from "../../agent_presets";
import type {
  AgentEvent,
  AgentRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamTaskRecord,
} from "../../api";

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

export const DEFAULT_TEAM_THREAD_TITLE = "all";

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

function isSharedThreadTask(task: TeamTaskRecord): boolean {
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
  selectedTaskId: string,
  teamId: string
): TeamTaskRecord | null {
  const teamTasks = sortTasksByActivity(tasks.filter((task) => task.team_id === teamId));
  const selectedId = selectedTaskId.trim();
  if (selectedId) {
    const selected = teamTasks.find((task) => task.id === selectedId);
    if (selected && isSharedThreadTask(selected)) {
      return selected;
    }
  }
  return teamTasks.find(isSharedThreadTask) ?? null;
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
