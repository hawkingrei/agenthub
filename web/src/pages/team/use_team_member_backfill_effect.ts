import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { AgentRecord } from "../../api";
import { api, getApiErrorStatus } from "../../api";

type UseTeamMemberBackfillEffectParams = {
  token: string;
  agents: AgentRecord[];
  teamSpecMemberIds: string[];
  teamMemberAgentsById: Record<string, AgentRecord | null>;
  setTeamMemberAgentsById: Dispatch<
    SetStateAction<Record<string, AgentRecord | null>>
  >;
};

type ResolvedTeamMemberAgent = {
  memberId: string;
  agent?: AgentRecord | null;
};

function stableSerializeRecord(value: unknown): string {
  return JSON.stringify(sortRecordValue(value));
}

function sortRecordValue(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortRecordValue);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, entryValue]) => [key, sortRecordValue(entryValue)])
    );
  }
  return value;
}

function areAgentRecordsEqual(
  left: AgentRecord | null | undefined,
  right: AgentRecord | null
): boolean {
  if (left === right) {
    return true;
  }
  if (left == null || right == null) {
    return left === right;
  }
  return stableSerializeRecord(left) === stableSerializeRecord(right);
}

export function useTeamMemberBackfillEffect({
  token,
  agents,
  teamSpecMemberIds,
  teamMemberAgentsById,
  setTeamMemberAgentsById,
}: UseTeamMemberBackfillEffectParams) {
  useEffect(() => {
    if (!token || teamSpecMemberIds.length === 0) {
      return;
    }
    const listedAgentIds = new Set(agents.map((agent) => agent.id));
    const unresolvedMemberIds = teamSpecMemberIds.filter((memberId) => {
      if (listedAgentIds.has(memberId)) {
        return false;
      }
      return teamMemberAgentsById[memberId] !== null;
    });
    if (unresolvedMemberIds.length === 0) {
      return;
    }

    let canceled = false;
    const loadMissingMemberAgents = async () => {
      const resolved: ResolvedTeamMemberAgent[] = await Promise.all(
        unresolvedMemberIds.map(async (memberId) => {
          try {
            return {
              memberId,
              agent: await api.getAgent(token, memberId),
            };
          } catch (err) {
            if (getApiErrorStatus(err) === 404) {
              return { memberId, agent: null };
            }
            return { memberId };
          }
        })
      );
      if (canceled) {
        return;
      }
      if (resolved.every(({ agent }) => agent === undefined)) {
        return;
      }
      setTeamMemberAgentsById((prev) => {
        let next: Record<string, AgentRecord | null> | null = null;
        for (const { memberId, agent } of resolved) {
          // Preserve prior cache on transient failures; only clear confirmed 404s.
          if (agent === undefined) {
            continue;
          }
          if (areAgentRecordsEqual(prev[memberId], agent)) {
            continue;
          }
          if (next === null) {
            next = { ...prev };
          }
          next[memberId] = agent;
        }
        return next ?? prev;
      });
    };

    void loadMissingMemberAgents();
    return () => {
      canceled = true;
    };
  }, [agents, teamMemberAgentsById, teamSpecMemberIds, token, setTeamMemberAgentsById]);
}
