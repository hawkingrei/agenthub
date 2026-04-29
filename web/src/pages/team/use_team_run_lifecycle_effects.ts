import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
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

const TEAM_RUN_CONTEXT_POLL_INTERVAL_MS = 4000;

function supportsLiveRunContext(tab: TeamTab): boolean {
  // Conversation, task, and ACP-heavy tabs already maintain their own live data paths.
  // Keep the run-context channel scoped to panes that actually render run snapshot metadata.
  return (
    tab === "runs" ||
    tab === "overview" ||
    tab === "events" ||
    tab === "steps" ||
    tab === "mailbox"
  );
}

function isEventDrivenRunContextTab(tab: TeamTab): boolean {
  return tab === "events" || tab === "steps" || tab === "overview" || tab === "mailbox";
}

type UseTeamRunLifecycleEffectsOptions = {
  token: string;
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
    token,
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
  const activeRunContextEnabled = supportsLiveRunContext(tab);
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);
  const pollFallbackAllowedRef = useRef(false);
  const sseConnectedRef = useRef(false);
  const sseConnectingRef = useRef(false);
  const [pollFallbackEnabled, setPollFallbackEnabled] = useState(false);

  useEffect(() => {
    hasCompletedInitialActiveRunRefreshRef.current = false;
  }, [activeRunIdForSelectedTeam, tab, chatInboxActorId]);

  const syncPollFallbackEnabled = useCallback(() => {
    const nextEnabled =
      pollFallbackAllowedRef.current &&
      (typeof EventSource === "undefined" ||
        (!sseConnectedRef.current && !sseConnectingRef.current));
    setPollFallbackEnabled((previous) =>
      previous === nextEnabled ? previous : nextEnabled
    );
  }, []);

  const refreshActiveRunContext = useCallback(async () => {
    const runId = activeRunIdForSelectedTeam?.trim() ?? "";
    if (!runId || !activeRunContextEnabled) {
      return;
    }
    if (
      typeof document !== "undefined" &&
      hasCompletedInitialActiveRunRefreshRef.current &&
      document.visibilityState !== "visible"
    ) {
      return;
    }
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      return;
    }
    refreshInFlightRef.current = true;
    try {
      for (;;) {
        refreshQueuedRef.current = false;
        if (tab === "mailbox") {
          await refreshSnapshot(runId).catch(() => undefined);
          const actorId = chatInboxActorId.trim();
          if (actorId) {
            await loadInbox(actorId).catch(() => undefined);
          }
        } else if (tab === "events") {
          await Promise.all([
            refreshEvents(runId).catch(() => undefined),
            refreshSnapshot(runId).catch(() => undefined),
          ]);
        } else if (tab === "runs") {
          await Promise.all([
            refreshRun(runId).catch(() => undefined),
            refreshSnapshot(runId).catch(() => undefined),
          ]);
        } else {
          await refreshSnapshot(runId).catch(() => undefined);
        }
        hasCompletedInitialActiveRunRefreshRef.current = true;
        if (!refreshQueuedRef.current) {
          return;
        }
      }
    } finally {
      refreshInFlightRef.current = false;
      if (refreshQueuedRef.current) {
        refreshQueuedRef.current = false;
        void refreshActiveRunContext().catch(() => undefined);
      }
    }
  }, [
    activeRunContextEnabled,
    activeRunIdForSelectedTeam,
    chatInboxActorId,
    loadInbox,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    tab,
  ]);

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
  }, [
    activeRunIdForSelectedTeam,
    setChatSeenByConversation,
    setChatStickToBottom,
    setEvents,
    setInbox,
    setMemberEvents,
    setSelectedMemberId,
    setSnapshot,
    setSteps,
  ]);

  const currentSnapshotRunId = snapshot?.run.id ?? null;
  const currentSnapshotTeamId = snapshot?.team.id ?? null;

  useEffect(() => {
    if (!activeRunIdForSelectedTeam || activeRunContextEnabled) {
      return;
    }
    if (
      currentSnapshotRunId === activeRunIdForSelectedTeam &&
      (!selectedTeamId || currentSnapshotTeamId === selectedTeamId)
    ) {
      return;
    }
    let canceled = false;
    const hydrateSnapshot = async () => {
      try {
        setError(null);
        await refreshSnapshot(activeRunIdForSelectedTeam);
      } catch (err) {
        if (!canceled) {
          setError(parseError(err));
        }
      }
    };
    void hydrateSnapshot();
    return () => {
      canceled = true;
    };
  }, [
    activeRunContextEnabled,
    activeRunIdForSelectedTeam,
    currentSnapshotRunId,
    currentSnapshotTeamId,
    parseError,
    refreshSnapshot,
    selectedTeamId,
    setError,
  ]);

  useEffect(() => {
    if (!activeRunIdForSelectedTeam || !activeRunContextEnabled) {
      return;
    }
    let canceled = false;
    const loadAll = async () => {
      try {
        setError(null);
        if (tab === "mailbox") {
          await refreshSnapshot(activeRunIdForSelectedTeam);
          return;
        }
        if (tab === "events") {
          await Promise.all([
            refreshEvents(activeRunIdForSelectedTeam),
            refreshSnapshot(activeRunIdForSelectedTeam),
          ]);
          return;
        }
        if (tab === "runs") {
          const run = await refreshRun(activeRunIdForSelectedTeam);
          if (canceled) return;
          if (selectedTeamId && run.team_id !== selectedTeamId) {
            setError(
              `Run ${run.id} belongs to team ${run.team_id}. Select that team to view it.`
            );
            setActiveRunId((current) => (current === run.id ? null : current));
            return;
          }
          await refreshSnapshot(activeRunIdForSelectedTeam);
          return;
        }
        await refreshSnapshot(activeRunIdForSelectedTeam);
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
    activeRunContextEnabled,
    activeRunIdForSelectedTeam,
    parseError,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    selectedTeamId,
    setActiveRunId,
    setError,
    tab,
  ]);

  useEffect(() => {
    pollFallbackAllowedRef.current = Boolean(
      eventsAutoRefresh &&
        activeRunContextEnabled &&
        activeRunIdForSelectedTeam?.trim() &&
        selectedTeamId?.trim()
    );
    syncPollFallbackEnabled();
  }, [
    activeRunContextEnabled,
    activeRunIdForSelectedTeam,
    eventsAutoRefresh,
    selectedTeamId,
    syncPollFallbackEnabled,
  ]);

  useEffect(() => {
    if (!eventsAutoRefresh || !activeRunContextEnabled || !activeRunIdForSelectedTeam) {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      setPollFallbackEnabled(false);
      return;
    }
    const normalizedTeamId = selectedTeamId?.trim() ?? "";
    const normalizedRunId = activeRunIdForSelectedTeam.trim();
    if (!normalizedTeamId) {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      setPollFallbackEnabled(false);
      return;
    }
    if (!isEventDrivenRunContextTab(tab) || typeof EventSource === "undefined" || !token.trim()) {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      syncPollFallbackEnabled();
      return;
    }
    let cancelled = false;
    let source: EventSource | null = null;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;

    const clearReconnectTimer = () => {
      if (reconnectTimer != null) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
    };

    const closeSource = () => {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      source?.close();
      source = null;
    };

    const scheduleReconnect = () => {
      if (cancelled) {
        return;
      }
      closeSource();
      syncPollFallbackEnabled();
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (!cancelled) {
          openSource();
        }
      }, delay);
    };

    function openSource() {
      closeSource();
      sseConnectingRef.current = true;
      syncPollFallbackEnabled();
      // The SSE payload is only a lightweight invalidation signal. The hook still
      // refreshes the tab-specific minimal read set so the route keeps one source of truth.
      const nextSource = new EventSource(
        `${location.origin}/sse/teams/${encodeURIComponent(
          normalizedTeamId
        )}/runs/${encodeURIComponent(normalizedRunId)}/context?token=${encodeURIComponent(
          token
        )}`
      );
      source = nextSource;
      nextSource.onopen = () => {
        reconnectAttempt = 0;
        sseConnectingRef.current = false;
        sseConnectedRef.current = true;
        syncPollFallbackEnabled();
      };
      nextSource.onmessage = (event) => {
        if (cancelled || event.data === "heartbeat") {
          return;
        }
        void refreshActiveRunContext().catch(() => undefined);
      };
      nextSource.onerror = () => {
        if (source !== nextSource) {
          nextSource.close();
          return;
        }
        scheduleReconnect();
      };
    }

    openSource();
    return () => {
      cancelled = true;
      clearReconnectTimer();
      closeSource();
      setPollFallbackEnabled(false);
    };
  }, [
    activeRunContextEnabled,
    activeRunIdForSelectedTeam,
    eventsAutoRefresh,
    refreshActiveRunContext,
    selectedTeamId,
    syncPollFallbackEnabled,
    tab,
    token,
  ]);

  useEffect(() => {
    if (
      !activeRunIdForSelectedTeam ||
      !eventsAutoRefresh ||
      !activeRunContextEnabled ||
      !pollFallbackEnabled
    ) {
      return;
    }
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
    }, TEAM_RUN_CONTEXT_POLL_INTERVAL_MS);
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
    activeRunContextEnabled,
    eventsAutoRefresh,
    pollFallbackEnabled,
    refreshActiveRunContext,
  ]);
}
