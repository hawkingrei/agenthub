import { useEffect } from "react";
import { isAgentActiveStatus } from "../../agent_ws";
import type { TeamTab } from "./state";

const TEAM_AGENT_ACP_SESSION_TABS = new Set<TeamTab>(["agent_acp", "member_console"]);
const ACTIVE_MEMBER_RUNTIME_STATUSES = new Set([
  "idle",
  "running",
  "working",
  "submitted",
  "input_required",
  "pending",
]);

export const TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS = 2000;

function isActiveMemberRuntimeStatus(status?: string | null): boolean {
  return ACTIVE_MEMBER_RUNTIME_STATUSES.has(status?.trim().toLowerCase() ?? "");
}

export function shouldRefreshSelectedAgentWorkspaceSession(args: {
  activeRunId: string | null | undefined;
  tab: TeamTab;
  selectedMemberId: string | null | undefined;
  selectedSessionId: string | null | undefined;
  snapshotStatus?: string | null;
  agentStatus?: string | null;
  runtimeSessionStatus?: string | null;
  runtimeAgentStatus?: string | null;
}): boolean {
  if (!args.activeRunId?.trim()) {
    return false;
  }
  if (!TEAM_AGENT_ACP_SESSION_TABS.has(args.tab)) {
    return false;
  }
  if (!args.selectedMemberId?.trim()) {
    return false;
  }
  if (args.selectedSessionId?.trim()) {
    return false;
  }
  const runtimeSessionStatus = args.runtimeSessionStatus?.trim().toLowerCase() ?? "";
  if (runtimeSessionStatus && !isActiveMemberRuntimeStatus(runtimeSessionStatus)) {
    return false;
  }
  const runtimeAgentStatus = args.runtimeAgentStatus?.trim().toLowerCase() ?? "";
  if (runtimeAgentStatus && !isActiveMemberRuntimeStatus(runtimeAgentStatus)) {
    return false;
  }
  return (
    isActiveMemberRuntimeStatus(args.snapshotStatus) ||
    isActiveMemberRuntimeStatus(runtimeAgentStatus) ||
    (!args.snapshotStatus?.trim() &&
      (isActiveMemberRuntimeStatus(args.agentStatus) ||
        isAgentActiveStatus(args.agentStatus ?? null)))
  );
}

type UseTeamMemberAcpSessionDiscoveryOptions = Parameters<
  typeof shouldRefreshSelectedAgentWorkspaceSession
>[0] & {
  refreshSnapshot: (runId: string) => Promise<unknown>;
};

export function useTeamMemberAcpSessionDiscovery({
  activeRunId,
  refreshSnapshot,
  ...args
}: UseTeamMemberAcpSessionDiscoveryOptions) {
  const shouldDiscoverSelectedAgentWorkspaceSession =
    shouldRefreshSelectedAgentWorkspaceSession({
      ...args,
      activeRunId,
    });

  useEffect(() => {
    const runId = activeRunId?.trim() ?? "";
    if (!shouldDiscoverSelectedAgentWorkspaceSession || !runId) {
      return;
    }
    let cancelled = false;
    const refreshSelectedMemberSession = () => {
      if (cancelled) {
        return;
      }
      void refreshSnapshot(runId).catch(() => undefined);
    };
    refreshSelectedMemberSession();
    const timer = window.setInterval(
      refreshSelectedMemberSession,
      TEAM_MEMBER_ACP_SESSION_DISCOVERY_INTERVAL_MS
    );
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [
    activeRunId,
    refreshSnapshot,
    shouldDiscoverSelectedAgentWorkspaceSession,
  ]);
}
