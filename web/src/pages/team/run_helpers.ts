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
