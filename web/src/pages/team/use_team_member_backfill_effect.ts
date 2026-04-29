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

const TEAM_MEMBER_BACKFILL_REVALIDATE_COOLDOWN_MS = 60_000;
const teamMemberLastResolvedAt = new Map<string, Map<string, number>>();
const teamMemberInFlightRequests = new Map<
  string,
  Map<string, Promise<ResolvedTeamMemberAgent>>
>();

function getSharedMemberMap<T>(
  cache: Map<string, Map<string, T>>,
  token: string,
  createIfMissing: boolean
): Map<string, T> | undefined {
  const existing = cache.get(token);
  if (existing || !createIfMissing) {
    return existing;
  }
  const next = new Map<string, T>();
  cache.set(token, next);
  return next;
}

// Shared caches keep duplicate Team shell instances from immediately re-fetching the
// same hidden member record before the first backfill result has propagated into state.
function getSharedResolvedAt(token: string, memberId: string): number | undefined {
  return getSharedMemberMap(teamMemberLastResolvedAt, token, false)?.get(memberId);
}

function setSharedResolvedAt(token: string, memberId: string, resolvedAt: number): void {
  getSharedMemberMap(teamMemberLastResolvedAt, token, true)?.set(memberId, resolvedAt);
}

function hasSharedInFlightRequest(token: string, memberId: string): boolean {
  return getSharedMemberMap(teamMemberInFlightRequests, token, false)?.has(memberId) ?? false;
}

function loadSharedMemberAgent(
  token: string,
  memberId: string
): Promise<ResolvedTeamMemberAgent> {
  const requestCache = getSharedMemberMap(teamMemberInFlightRequests, token, true)!;
  const existing = requestCache.get(memberId);
  if (existing) {
    return existing;
  }
  const request = api
    .getAgent(token, memberId)
    .then((agent) => ({ memberId, agent }))
    .catch((err) => {
      if (getApiErrorStatus(err) === 404) {
        return { memberId, agent: null };
      }
      return { memberId };
    })
    .finally(() => {
      requestCache.delete(memberId);
      if (requestCache.size === 0) {
        teamMemberInFlightRequests.delete(token);
      }
    });
  requestCache.set(memberId, request);
  return request;
}

export function resetTeamMemberBackfillCachesForTest(): void {
  teamMemberLastResolvedAt.clear();
  teamMemberInFlightRequests.clear();
}

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
    const now = Date.now();
    const unresolvedMemberIds = teamSpecMemberIds.filter((memberId) => {
      if (listedAgentIds.has(memberId)) {
        return false;
      }
      const lastResolvedAt = getSharedResolvedAt(token, memberId);
      const withinSharedCooldown =
        lastResolvedAt != null &&
        now - lastResolvedAt < TEAM_MEMBER_BACKFILL_REVALIDATE_COOLDOWN_MS;
      if (!Object.prototype.hasOwnProperty.call(teamMemberAgentsById, memberId)) {
        return !withinSharedCooldown && !hasSharedInFlightRequest(token, memberId);
      }
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
        unresolvedMemberIds.map((memberId) => loadSharedMemberAgent(token, memberId))
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
          setSharedResolvedAt(token, memberId, resolvedAt);
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
