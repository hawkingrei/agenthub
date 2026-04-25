import { useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import {
  AgentEvent,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../../api";
import { resolveActiveRunIdForSelectedTeam, type TeamRunStatusFilter } from "./run_helpers";
import type { TeamTab } from "./state";

function shouldPollActiveRunContext(tab: TeamTab): boolean {
  return tab !== "agent_acp" && tab !== "member_console" && tab !== "mailbox";
}

type UseTeamRunLifecycleEffectsOptions = {
  selectedTeamId: string | null;
  runStatusFilter: TeamRunStatusFilter;
  runs: TeamRunRecord[];
  activeRunIdForSelectedTeam: string | null;
  snapshot: TeamRunSnapshotRecord | null;
  eventsAutoRefresh: boolean;
  tab: TeamTab;
  chatInboxActorId: string;
  refreshAgents: () => Promise<unknown>;
  refreshTeams: () => Promise<void>;
  refreshTeamRuns: (
    teamId: string,
    mode?: "replace" | "append",
    options?: {
      statusFilter?: TeamRunStatusFilter;
      beforeCreatedAt?: number;
    }
  ) => Promise<unknown>;
  refreshRun: (runId: string) => Promise<TeamRunRecord>;
  refreshSteps: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string) => Promise<void>;
  refreshSnapshot: (runId: string) => Promise<TeamRunSnapshotRecord>;
  loadInbox: (actorIdOverride?: string) => Promise<void>;
  parseError: (err: unknown) => string;
  setError: Dispatch<SetStateAction<string | null>>;
  setActiveRunId: Dispatch<SetStateAction<string | null>>;
  setRuns: Dispatch<SetStateAction<TeamRunRecord[]>>;
  setEvents: Dispatch<SetStateAction<TeamRunEventRecord[]>>;
  setSteps: Dispatch<SetStateAction<TeamStepRecord[]>>;
  setInbox: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
  setSnapshot: Dispatch<SetStateAction<TeamRunSnapshotRecord | null>>;
  setSelectedMemberId: Dispatch<SetStateAction<string>>;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setChatSeenByConversation: Dispatch<SetStateAction<Record<string, number>>>;
  setChatStickToBottom: (next: boolean) => void;
};

export function useTeamRunLifecycleEffects(options: UseTeamRunLifecycleEffectsOptions) {
  const {
    selectedTeamId,
    runStatusFilter,
    runs,
    activeRunIdForSelectedTeam,
    snapshot,
    eventsAutoRefresh,
    tab,
    chatInboxActorId,
    refreshAgents,
    refreshTeams,
    refreshTeamRuns,
    refreshRun,
    refreshSteps,
    refreshEvents,
    refreshSnapshot,
    loadInbox,
    parseError,
    setError,
    setActiveRunId,
    setRuns,
    setEvents,
    setSteps,
    setInbox,
    setSnapshot,
    setSelectedMemberId,
    setMemberEvents,
    setChatSeenByConversation,
    setChatStickToBottom,
  } = options;
  const hasCompletedInitialActiveRunRefreshRef = useRef(false);
  const activeRunContextPollingEnabled = shouldPollActiveRunContext(tab);

  useEffect(() => {
    hasCompletedInitialActiveRunRefreshRef.current = false;
  }, [activeRunIdForSelectedTeam, tab, chatInboxActorId]);

  useEffect(() => {
    void refreshTeams();
    void refreshAgents().catch((err) => {
      setError(parseError(err));
    });
  }, [parseError, refreshAgents, refreshTeams, setError]);

  useEffect(() => {
    if (!selectedTeamId) {
      setActiveRunId(null);
      setRuns([]);
      setEvents([]);
      setSteps([]);
      setInbox([]);
      setSnapshot(null);
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    let canceled = false;
    const loadTeamRuns = async () => {
      try {
        setError(null);
        await refreshTeamRuns(selectedTeamId, "replace", {
          statusFilter: runStatusFilter,
        });
        if (canceled) return;
      } catch (err) {
        if (!canceled) {
          setError(parseError(err));
        }
      }
    };
    void loadTeamRuns();
    return () => {
      canceled = true;
    };
  }, [
    parseError,
    refreshTeamRuns,
    runStatusFilter,
    selectedTeamId,
    setActiveRunId,
    setError,
    setEvents,
    setInbox,
    setMemberEvents,
    setRuns,
    setSelectedMemberId,
    setSnapshot,
    setSteps,
  ]);

  useEffect(() => {
    setActiveRunId((prev) =>
      resolveActiveRunIdForSelectedTeam(runs, selectedTeamId, prev)
    );
  }, [runs, selectedTeamId, setActiveRunId]);

  useEffect(() => {
    if (!activeRunIdForSelectedTeam) {
      setEvents([]);
      setSteps([]);
      setInbox([]);
      setSnapshot(null);
      setSelectedMemberId("");
      setMemberEvents([]);
      setChatSeenByConversation({});
      setChatStickToBottom(true);
      return;
    }
    let canceled = false;
    const loadAll = async () => {
      try {
        setError(null);
        if (!activeRunContextPollingEnabled) {
          const currentSnapshotRunId = snapshot?.run.id ?? null;
          const currentSnapshotTeamId = snapshot?.team.id ?? null;
          if (
            currentSnapshotRunId === activeRunIdForSelectedTeam &&
            (!selectedTeamId || currentSnapshotTeamId === selectedTeamId)
          ) {
            return;
          }
          await refreshSnapshot(activeRunIdForSelectedTeam);
          return;
        }
        const run = await refreshRun(activeRunIdForSelectedTeam);
        if (canceled) return;
        if (selectedTeamId && run.team_id !== selectedTeamId) {
          setError(
            `Run ${run.id} belongs to team ${run.team_id}. Select that team to view it.`
          );
          setActiveRunId((current) => (current === run.id ? null : current));
          return;
        }
        await Promise.all([
          refreshSteps(activeRunIdForSelectedTeam),
          refreshEvents(activeRunIdForSelectedTeam),
          refreshSnapshot(activeRunIdForSelectedTeam),
        ]);
      } catch (err) {
        if (!canceled) {
          setError(parseError(err));
        }
      }
    };
    void loadAll();
    return () => {
      canceled = true;
    };
  }, [
    activeRunContextPollingEnabled,
    activeRunIdForSelectedTeam,
    parseError,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    refreshSteps,
    selectedTeamId,
    setActiveRunId,
    setChatSeenByConversation,
    setChatStickToBottom,
    setError,
    setEvents,
    setInbox,
    setMemberEvents,
    setSelectedMemberId,
    setSnapshot,
    setSteps,
    snapshot,
  ]);

  useEffect(() => {
    if (
      !activeRunIdForSelectedTeam ||
      !eventsAutoRefresh ||
      !activeRunContextPollingEnabled
    ) {
      return;
    }
    const refreshActiveRunContext = async () => {
      if (
        typeof document !== "undefined" &&
        hasCompletedInitialActiveRunRefreshRef.current &&
        document.visibilityState !== "visible"
      ) {
        return;
      }
      if (tab === "mailbox") {
        await refreshSnapshot(activeRunIdForSelectedTeam).catch(() => undefined);
        const actorId = chatInboxActorId.trim();
        if (actorId) {
          await loadInbox(actorId).catch(() => undefined);
        }
        hasCompletedInitialActiveRunRefreshRef.current = true;
        return;
      }
      await Promise.all([
        refreshRun(activeRunIdForSelectedTeam).catch(() => undefined),
        refreshEvents(activeRunIdForSelectedTeam).catch(() => undefined),
        refreshSnapshot(activeRunIdForSelectedTeam).catch(() => undefined),
      ]);
      hasCompletedInitialActiveRunRefreshRef.current = true;
    };
    const handleFocus = () => {
      void refreshActiveRunContext();
    };
    const handleOnline = () => {
      void refreshActiveRunContext();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        void refreshActiveRunContext();
      }
    };
    const timer = window.setInterval(() => {
      void refreshActiveRunContext();
    }, 4000);
    window.addEventListener("focus", handleFocus);
    window.addEventListener("online", handleOnline);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("online", handleOnline);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [
    activeRunIdForSelectedTeam,
    activeRunContextPollingEnabled,
    chatInboxActorId,
    eventsAutoRefresh,
    loadInbox,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    tab,
  ]);
}
