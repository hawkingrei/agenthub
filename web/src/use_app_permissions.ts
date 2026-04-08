import { useEffect, useMemo, type Dispatch, type SetStateAction } from "react";
import { AuthState } from "./types";
import { AgentRecord, api, AcpPermissionRecord } from "./api";
import { 
  buildPermissionPollAgentIds, 
  isSamePendingPermissionCountMap, 
  isSamePermissionList, 
} from "./app_agents_helpers";
import {
  parsePermissionPollAgentIds, 
  buildGlobalPermissionPollAgentIds, 
  chunkPermissionPollAgentIds, 
  resolveGlobalPermissionPollIntervalMs, 
  mergePendingPermissionCountMap, 
  schedulePermissionPollLoop, 
  filterPermissionsForAgent 
} from "./app_permission_polling";

const GLOBAL_PERMISSION_POLL_MAX_CONCURRENCY = 4;

export function useAppPermissions(
  auth: AuthState | null,
  isAgentsRoute: boolean,
  agents: AgentRecord[],
  activeAgent: string | null,
  agentsCollapsed: boolean,
  developerMode: boolean,
  acpTab: string,
  state: {
    acpPermissions: AcpPermissionRecord[];
    setAcpPermissions: Dispatch<SetStateAction<AcpPermissionRecord[]>>;
    pendingPermissionCounts: Record<string, number>;
    setPendingPermissionCounts: Dispatch<SetStateAction<Record<string, number>>>;
    acpPermissionHistory: AcpPermissionRecord[];
    setAcpPermissionHistory: Dispatch<SetStateAction<AcpPermissionRecord[]>>;
  }
) {
  const token = auth?.token ?? null;
  const { 
    acpPermissions, 
    setAcpPermissions, 
    pendingPermissionCounts, 
    setPendingPermissionCounts, 
    acpPermissionHistory, 
    setAcpPermissionHistory 
  } = state;

  const permissionPollAgentIds = useMemo(() => buildPermissionPollAgentIds(agents), [agents]);
  const permissionPollAgentIdsKey = useMemo(() => permissionPollAgentIds.join(","), [permissionPollAgentIds]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !permissionPollAgentIdsKey) {
      setPendingPermissionCounts({});
      return;
    }
    let cancelled = false;
    const allAgentIds = parsePermissionPollAgentIds(permissionPollAgentIdsKey);
    const requestedAgentIds = buildGlobalPermissionPollAgentIds(allAgentIds, activeAgent);
    const requestedChunks = chunkPermissionPollAgentIds(requestedAgentIds, GLOBAL_PERMISSION_POLL_MAX_CONCURRENCY);
    const pollIntervalMs = resolveGlobalPermissionPollIntervalMs(agentsCollapsed);

    const load = async () => {
      const entries: Array<readonly [string, number | null]> = [];
      for (const chunk of requestedChunks) {
        const batch = await Promise.all(
          chunk.map(async (agentId) => {
            try {
              const items = await api.listAcpPermissions(token, agentId, "pending");
              return [agentId, items.length] as const;
            } catch {
              return [agentId, null] as const;
            }
          })
        );
        entries.push(...batch);
        if (cancelled) return;
      }
      if (cancelled) return;
      setPendingPermissionCounts((prev) => {
        const nextCounts = mergePendingPermissionCountMap(prev, allAgentIds, entries);
        return isSamePendingPermissionCountMap(prev, nextCounts) ? prev : nextCounts;
      });
    };

    load();
    const timer = window.setInterval(load, pollIntervalMs);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [isAgentsRoute, token, permissionPollAgentIdsKey, activeAgent, agentsCollapsed, setPendingPermissionCounts]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent) {
      setAcpPermissions([]);
      return;
    }
    let cancelled = false;
    const requestedAgentId = activeAgent;
    const pollState = { timer: null as number | null };
    
    const pollOnce = async (): Promise<number> => {
      try {
        const items = await api.listAcpPermissions(token, requestedAgentId, "pending");
        if (!cancelled && activeAgent === requestedAgentId) {
          setAcpPermissions((prev) => (isSamePermissionList(prev, items) ? prev : items));
          setPendingPermissionCounts((prev) => {
            const nextCount = items.length;
            if (nextCount <= 0) {
              if (!(requestedAgentId in prev)) return prev;
              const next = { ...prev };
              delete next[requestedAgentId];
              return next;
            }
            if (prev[requestedAgentId] === nextCount) return prev;
            return { ...prev, [requestedAgentId]: nextCount };
          });
        }
        return items.length;
      } catch {
        if (!cancelled) setAcpPermissions([]);
        return 0;
      }
    };

    const schedule = (delay: number) => {
      schedulePermissionPollLoop(delay, pollState, pollOnce, () => cancelled);
    };
    schedule(0);
    return () => {
      cancelled = true;
      if (pollState.timer) window.clearTimeout(pollState.timer);
    };
  }, [isAgentsRoute, token, activeAgent, setAcpPermissions, setPendingPermissionCounts]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent || !developerMode || acpTab !== "debug") {
      setAcpPermissionHistory([]);
      return;
    }
    let cancelled = false;
    const requestedAgentId = activeAgent;
    const load = async () => {
      try {
        const items = await api.listAcpPermissions(token, requestedAgentId);
        if (!cancelled && activeAgent === requestedAgentId) {
          setAcpPermissionHistory((prev) => (isSamePermissionList(prev, items) ? prev : items));
        }
      } catch {
        if (!cancelled) setAcpPermissionHistory([]);
      }
    };
    load();
    const timer = window.setInterval(load, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [isAgentsRoute, token, activeAgent, developerMode, acpTab, setAcpPermissionHistory]);

  const scopedAcpPermissions = useMemo(() => filterPermissionsForAgent(acpPermissions, activeAgent), [acpPermissions, activeAgent]);
  const scopedAcpPermissionHistory = useMemo(() => filterPermissionsForAgent(acpPermissionHistory, activeAgent), [acpPermissionHistory, activeAgent]);

  return {
    acpPermissions,
    pendingPermissionCounts,
    acpPermissionHistory,
    scopedAcpPermissions,
    scopedAcpPermissionHistory,
  };
}
