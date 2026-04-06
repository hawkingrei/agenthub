import { useEffect, useRef } from "react";
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

const TEAM_MEMBER_BACKFILL_REVALIDATE_COOLDOWN_MS = 60_000;

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
  const lastResolvedAtRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    const memberIdSet = new Set(teamSpecMemberIds);
    if (lastResolvedAtRef.current.size > 0) {
      for (const memberId of lastResolvedAtRef.current.keys()) {
        if (!memberIdSet.has(memberId)) {
          lastResolvedAtRef.current.delete(memberId);
        }
      }
    }
    if (!token || teamSpecMemberIds.length === 0) {
      return;
    }
    const listedAgentIds = new Set(agents.map((agent) => agent.id));
    const now = Date.now();
    const unresolvedMemberIds = teamSpecMemberIds.filter((memberId) => {
      if (listedAgentIds.has(memberId)) {
        return false;
      }
      if (!Object.prototype.hasOwnProperty.call(teamMemberAgentsById, memberId)) {
        return true;
      }
      const lastResolvedAt = lastResolvedAtRef.current.get(memberId);
      return (
        lastResolvedAt == null ||
        now - lastResolvedAt >= TEAM_MEMBER_BACKFILL_REVALIDATE_COOLDOWN_MS
      );
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
      const resolvedAt = Date.now();
      for (const { memberId, agent } of resolved) {
        if (agent !== undefined) {
          lastResolvedAtRef.current.set(memberId, resolvedAt);
        }
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
