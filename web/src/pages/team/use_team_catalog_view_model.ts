import { useDeferredValue, useMemo } from "react";
import type {
  AgentRecord,
  TeamDefinitionRecord,
  TeamRuntimeRecord,
  TeamRunSnapshotRecord,
} from "../../api";
import {
  buildTeamMemberLiveStates,
  parseTeamSpecMembers,
  resolveTeamMemberAgentStatusesFromMembers,
  summarizeTeamMemberAgentStatuses,
  type TeamMemberAgentStatus,
  type TeamMemberAgentStatusSummary,
  type TeamMemberLiveState,
  type TeamSpecMember,
} from "./member_helpers";
import { resolveTeamRuntimeControlTone, resolveTeamRuntimeStatus } from "./page_helpers";
import type { TeamSelectorItem } from "./team_selector_panel";

type UseTeamCatalogViewModelOptions = {
  teams: TeamDefinitionRecord[];
  agents: AgentRecord[];
  teamMemberAgentsById: Record<string, AgentRecord | null>;
  teamRuntimeByTeamId: Record<string, TeamRuntimeRecord | null | undefined>;
  selectedTeam: TeamDefinitionRecord | null;
  snapshot: TeamRunSnapshotRecord | null;
  teamSelectorFilter: string;
};

type TeamRuntimeStatusView = ReturnType<typeof resolveTeamRuntimeStatus>;

export function useTeamCatalogViewModel({
  teams,
  agents,
  teamMemberAgentsById,
  teamRuntimeByTeamId,
  selectedTeam,
  snapshot,
  teamSelectorFilter,
}: UseTeamCatalogViewModelOptions) {
  const teamSpecMembersByTeamId = useMemo(() => {
    const next = new Map<string, TeamSpecMember[]>();
    for (const team of teams) {
      next.set(team.id, parseTeamSpecMembers(team.spec));
    }
    return next;
  }, [teams]);

  const teamSpecMemberIds = useMemo(() => {
    const ids = new Set<string>();
    for (const members of teamSpecMembersByTeamId.values()) {
      for (const member of members) {
        ids.add(member.member_id);
      }
    }
    return [...ids];
  }, [teamSpecMembersByTeamId]);

  const teamMemberStatusByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatus[]>();
    for (const team of teams) {
      next.set(
        team.id,
        resolveTeamMemberAgentStatusesFromMembers(
          teamSpecMembersByTeamId.get(team.id) ?? [],
          agents,
          teamMemberAgentsById,
          teamRuntimeByTeamId[team.id]?.members
        )
      );
    }
    return next;
  }, [agents, teamMemberAgentsById, teamRuntimeByTeamId, teamSpecMembersByTeamId, teams]);

  const teamMemberSummaryByTeamId = useMemo(() => {
    const next = new Map<string, TeamMemberAgentStatusSummary>();
    for (const team of teams) {
      next.set(
        team.id,
        summarizeTeamMemberAgentStatuses(teamMemberStatusByTeamId.get(team.id) ?? [])
      );
    }
    return next;
  }, [teamMemberStatusByTeamId, teams]);

  const deferredTeamSelectorFilter = useDeferredValue(teamSelectorFilter);
  const normalizedTeamSelectorFilter = deferredTeamSelectorFilter.trim().toLowerCase();

  const selectorVisibleTeams = useMemo(() => {
    if (!normalizedTeamSelectorFilter) {
      return teams;
    }
    return teams.filter((team) => {
      const name = team.name.toLowerCase();
      const id = team.id.toLowerCase();
      return name.includes(normalizedTeamSelectorFilter) || id.includes(normalizedTeamSelectorFilter);
    });
  }, [normalizedTeamSelectorFilter, teams]);

  const teamRuntimeStatusByTeamId = useMemo(() => {
    const next = new Map<string, TeamRuntimeStatusView>();
    for (const team of teams) {
      next.set(
        team.id,
        resolveTeamRuntimeStatus(
          teamMemberSummaryByTeamId.get(team.id) ?? null,
          teamRuntimeByTeamId[team.id] ?? null
        )
      );
    }
    return next;
  }, [teamMemberSummaryByTeamId, teamRuntimeByTeamId, teams]);

  const selectorTeamItems = useMemo<TeamSelectorItem[]>(
    () =>
      selectorVisibleTeams.map((team) => {
        const summary = teamMemberSummaryByTeamId.get(team.id);
        const runtimeStatus = teamRuntimeStatusByTeamId.get(team.id);
        return {
          id: team.id,
          name: team.name,
          description: team.description?.trim() || "No mission summary yet.",
          summary: summary
            ? `${summary.total} members · ${summary.active} active${
                summary.inactive > 0 ? ` · ${summary.inactive} idle` : ""
              }${summary.missing > 0 ? ` · ${summary.missing} missing` : ""}`
            : "No agents configured yet",
          runtimeLabel: runtimeStatus?.label ?? "stopped",
        };
      }),
    [selectorVisibleTeams, teamMemberSummaryByTeamId, teamRuntimeStatusByTeamId]
  );

  const selectedTeamMemberStatuses = useMemo(() => {
    if (!selectedTeam) {
      return [];
    }
    return teamMemberStatusByTeamId.get(selectedTeam.id) ?? [];
  }, [selectedTeam, teamMemberStatusByTeamId]);

  const selectedTeamSnapshotMembers = useMemo(() => {
    if (!selectedTeam || !snapshot) {
      return undefined;
    }
    if (snapshot.team.id !== selectedTeam.id) {
      return undefined;
    }
    return snapshot.members;
  }, [selectedTeam, snapshot]);

  const selectedTeamMemberLiveStates = useMemo<TeamMemberLiveState[]>(
    () =>
      buildTeamMemberLiveStates(selectedTeamMemberStatuses, selectedTeamSnapshotMembers),
    [selectedTeamMemberStatuses, selectedTeamSnapshotMembers]
  );

  const selectedTeamMemberSummary = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return teamMemberSummaryByTeamId.get(selectedTeam.id) ?? null;
  }, [selectedTeam, teamMemberSummaryByTeamId]);

  const selectedTeamRuntime = useMemo(() => {
    if (!selectedTeam) {
      return null;
    }
    return teamRuntimeByTeamId[selectedTeam.id] ?? null;
  }, [selectedTeam, teamRuntimeByTeamId]);

  const selectedTeamRuntimeStatus = useMemo(
    () => resolveTeamRuntimeStatus(selectedTeamMemberSummary, selectedTeamRuntime),
    [selectedTeamMemberSummary, selectedTeamRuntime]
  );

  const selectedTeamRuntimeControlTone = useMemo(
    () => resolveTeamRuntimeControlTone(selectedTeamRuntimeStatus.status),
    [selectedTeamRuntimeStatus.status]
  );

  const selectedTeamMembers = useMemo(() => {
    if (!selectedTeam) {
      return [];
    }
    return teamSpecMembersByTeamId.get(selectedTeam.id) ?? [];
  }, [selectedTeam, teamSpecMembersByTeamId]);

  const selectedTeamHasConfiguredMembers = selectedTeamMembers.length > 0;
  const selectedTeamHasCoordinator = useMemo(
    () => selectedTeamMembers.some((member) => member.role === "coordinator"),
    [selectedTeamMembers]
  );
  const selectedTeamWorkerCount = useMemo(
    () => selectedTeamMembers.filter((member) => member.role === "worker").length,
    [selectedTeamMembers]
  );

  return {
    teamSpecMemberIds,
    teamMemberStatusByTeamId,
    teamMemberSummaryByTeamId,
    selectorTeamItems,
    selectedTeamMemberStatuses,
    selectedTeamSnapshotMembers,
    selectedTeamMemberLiveStates,
    selectedTeamMemberSummary,
    selectedTeamRuntime,
    selectedTeamRuntimeStatus,
    selectedTeamRuntimeControlTone,
    selectedTeamMembers,
    selectedTeamHasConfiguredMembers,
    selectedTeamHasCoordinator,
    selectedTeamWorkerCount,
  };
}
