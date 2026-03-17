import { TeamRunEventRecord, TeamRunRecord, TeamRunStatus } from "../../api";

const TEAM_EVENT_PREVIEW_LIMIT = 5;

export type TeamRunStatusFilter = TeamRunStatus | "all";

function sortRuns(runs: TeamRunRecord[]): TeamRunRecord[] {
  return [...runs].sort((a, b) => b.created_at - a.created_at);
}

export function mergeRunPages(
  existing: TeamRunRecord[],
  incoming: TeamRunRecord[]
): TeamRunRecord[] {
  const byId = new Map<string, TeamRunRecord>();
  for (const run of existing) {
    byId.set(run.id, run);
  }
  for (const run of incoming) {
    byId.set(run.id, run);
  }
  return sortRuns([...byId.values()]);
}

export function mergeTeamRunList(
  previousTeamRuns: TeamRunRecord[],
  incoming: TeamRunRecord[],
  mode: "replace" | "append",
  activeRunId: string | null
): TeamRunRecord[] {
  const base = mode === "append" ? previousTeamRuns : [];
  let merged = mergeRunPages(base, incoming);
  if (mode !== "replace" || !activeRunId) {
    return merged;
  }
  const pinned = previousTeamRuns.find((run) => run.id === activeRunId);
  if (!pinned || merged.some((run) => run.id === pinned.id)) {
    return merged;
  }
  merged = mergeRunPages(merged, [pinned]);
  return merged;
}

export function resolveRunStatusFilter(
  status: TeamRunStatusFilter
): TeamRunStatus | undefined {
  return status === "all" ? undefined : status;
}

export function selectTeamPreviewEvents(
  events: TeamRunEventRecord[],
  selectedMemberId: string,
  limit = TEAM_EVENT_PREVIEW_LIMIT
): TeamRunEventRecord[] {
  if (selectedMemberId.trim().length > 0) {
    return events;
  }
  if (events.length <= limit) {
    return events;
  }
  return events.slice(events.length - limit);
}

export function extractTaskIdFromRunInput(input: unknown): string | null {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    return null;
  }
  const rawTaskId = (input as { task_id?: unknown }).task_id;
  return typeof rawTaskId === "string" && rawTaskId.trim().length > 0
    ? rawTaskId.trim()
    : null;
}

export function selectRunsForTask(runs: TeamRunRecord[], taskId: string): TeamRunRecord[] {
  const normalizedTaskId = taskId.trim();
  if (!normalizedTaskId) {
    return [];
  }
  return runs
    .filter((run) => extractTaskIdFromRunInput(run.input) === normalizedTaskId)
    .sort((left, right) => right.created_at - left.created_at);
}

export function resolveActiveRunIdForSelectedTeam(
  runs: TeamRunRecord[],
  selectedTeamId: string | null,
  currentActiveRunId: string | null
): string | null {
  if (!selectedTeamId) {
    return null;
  }
  if (
    currentActiveRunId &&
    runs.some((run) => run.id === currentActiveRunId && run.team_id === selectedTeamId)
  ) {
    return currentActiveRunId;
  }
  return runs.find((run) => run.team_id === selectedTeamId)?.id ?? null;
}
