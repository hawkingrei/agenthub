import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../../api";
import type { TeamRunStatusFilter } from "./run_helpers";
import type { TeamTab } from "./state";

type RefreshTeamRuns = (
  teamId: string,
  mode?: "replace" | "append",
  options?: {
    statusFilter?: TeamRunStatusFilter;
    beforeCreatedAt?: number;
  }
) => Promise<unknown>;

type UseTeamRunEffectsParams = {
  selectedTeamId: string | null;
  runs: TeamRunRecord[];
  activeRunId: string | null;
  runStatusFilter: TeamRunStatusFilter;
  eventsAutoRefresh: boolean;
  tab: TeamTab;
  chatInboxActorId: string;
  refreshTeams: () => Promise<unknown>;
  refreshAgents: () => Promise<unknown>;
  refreshTeamRuns: RefreshTeamRuns;
  refreshRun: (runId: string) => Promise<TeamRunRecord>;
  refreshSteps: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<unknown>;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  loadInbox: (actorIdOverride?: string) => Promise<unknown>;
  parseErrorMessage: (error: unknown) => string;
  setError: (value: string | null) => void;
  setSelectedTeamId: (value: string | null) => void;
  setActiveRunId: Dispatch<SetStateAction<string | null>>;
  setRuns: Dispatch<SetStateAction<TeamRunRecord[]>>;
  setEvents: Dispatch<SetStateAction<TeamRunEventRecord[]>>;
  setSteps: Dispatch<SetStateAction<TeamStepRecord[]>>;
  setInbox: (next: TeamActorMessageRecord[]) => void;
  setSnapshot: Dispatch<SetStateAction<TeamRunSnapshotRecord | null>>;
  setSelectedMemberId: (next: string) => void;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setChatSeenByConversation: (next: Record<string, number>) => void;
  setChatStickToBottom: (next: boolean) => void;
};

export function useTeamRunEffects({
  selectedTeamId,
  runs,
  activeRunId,
  runStatusFilter,
  eventsAutoRefresh,
  tab,
  chatInboxActorId,
  refreshTeams,
  refreshAgents,
  refreshTeamRuns,
  refreshRun,
  refreshSteps,
  refreshEvents,
  refreshSnapshot,
  loadInbox,
  parseErrorMessage,
  setError,
  setSelectedTeamId,
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
}: UseTeamRunEffectsParams) {
  useEffect(() => {
    void refreshTeams();
    void refreshAgents().catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [parseErrorMessage, refreshAgents, refreshTeams, setError]);

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
          setError(parseErrorMessage(err));
        }
      }
    };
    void loadTeamRuns();
    return () => {
      canceled = true;
    };
  }, [
    parseErrorMessage,
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
    if (!selectedTeamId) return;
    setActiveRunId((prev) => {
      if (prev && runs.some((run) => run.id === prev && run.team_id === selectedTeamId)) {
        return prev;
      }
      return runs.find((run) => run.team_id === selectedTeamId)?.id ?? null;
    });
  }, [runs, selectedTeamId, setActiveRunId]);

  useEffect(() => {
    if (!activeRunId) {
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
        const run = await refreshRun(activeRunId);
        if (canceled) return;
        if (run.team_id !== selectedTeamId) {
          setSelectedTeamId(run.team_id);
        }
        await Promise.all([
          refreshSteps(activeRunId),
          refreshEvents(activeRunId),
          refreshSnapshot(activeRunId),
        ]);
      } catch (err) {
        if (!canceled) {
          setError(parseErrorMessage(err));
        }
      }
    };
    void loadAll();
    return () => {
      canceled = true;
    };
  }, [
    activeRunId,
    parseErrorMessage,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    refreshSteps,
    selectedTeamId,
    setChatSeenByConversation,
    setChatStickToBottom,
    setError,
    setEvents,
    setInbox,
    setMemberEvents,
    setSelectedMemberId,
    setSelectedTeamId,
    setSnapshot,
    setSteps,
  ]);

  useEffect(() => {
    if (!activeRunId || !eventsAutoRefresh) return;
    const timer = window.setInterval(() => {
      if (tab === "mailbox") {
        void refreshSnapshot(activeRunId).catch(() => undefined);
        const actorId = chatInboxActorId.trim();
        if (actorId) {
          void loadInbox(actorId).catch(() => undefined);
        }
        return;
      }
      void refreshRun(activeRunId).catch(() => undefined);
      void refreshEvents(activeRunId).catch(() => undefined);
      void refreshSnapshot(activeRunId).catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    activeRunId,
    chatInboxActorId,
    eventsAutoRefresh,
    loadInbox,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    tab,
  ]);
}
