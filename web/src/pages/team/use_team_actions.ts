import {
  useCallback,
  useMemo,
  useRef,
  type Dispatch,
  type MutableRefObject,
  type SetStateAction,
} from "react";
import {
  type AgentEvent,
  type AgentRecord,
  api,
  getTeamStepRuntimeHandleId,
  type TeamActorMessageRecord,
  type TeamDefinitionRecord,
  type TeamMemberSnapshot,
  type TeamRunEventRecord,
  type TeamRunRecord,
  type TeamRunSnapshotRecord,
  type TeamStepRecord,
} from "../../api";
import {
  parseErrorMessage,
  parseOptionalInteger,
  parseOptionalJson,
} from "./create_helpers";
import { upsertAgentEventList, upsertEventList, upsertRun } from "./page_helpers";
import {
  mergeRunPages,
  mergeTeamRunList,
  resolveRunStatusFilter,
  type TeamRunStatusFilter,
} from "./run_helpers";
import {
  EVENT_PAGE_LIMIT,
  MEMBER_EVENT_PAGE_LIMIT,
  TEAM_RUN_PAGE_LIMIT,
  type TeamRunBrowserState,
} from "./state";

type UseTeamActionsOptions = {
  token: string;
  selectedTeamId: string | null;
  runContextId: string;
  runInput: string;
  runLookupId: string;
  runStatusFilter: TeamRunStatusFilter;
  runsLoading: boolean;
  runsHasMore: boolean;
  runsBeforeCreatedAt?: number;
  selectedStepId: string;
  activeRunIdForSelectedTeam: string | null;
  activeRunForSelectedTeam: TeamRunRecord | null;
  inboxActorId: string;
  inboxLimit: string;
  inboxAfterId: string;
  inboxIncludeDelivered: boolean;
  selectedMemberAgentId: string | null;
  selectedMemberSessionId: string | null;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  activeRunIdRef: MutableRefObject<string | null>;
  eventsRef: MutableRefObject<TeamRunEventRecord[]>;
  memberEventsRef: MutableRefObject<AgentEvent[]>;
  setBusy: Dispatch<SetStateAction<string | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setAgents: Dispatch<SetStateAction<AgentRecord[]>>;
  setTeams: Dispatch<SetStateAction<TeamDefinitionRecord[]>>;
  setSelectedTeamId: Dispatch<SetStateAction<string | null>>;
  setRuns: Dispatch<SetStateAction<TeamRunRecord[]>>;
  setTeamRunBrowserByTeam: Dispatch<SetStateAction<Record<string, TeamRunBrowserState>>>;
  setRunsLoading: Dispatch<SetStateAction<boolean>>;
  setSteps: Dispatch<SetStateAction<TeamStepRecord[]>>;
  setSelectedStepId: (next: string) => void;
  setEvents: Dispatch<SetStateAction<TeamRunEventRecord[]>>;
  setEventsLoading: Dispatch<SetStateAction<boolean>>;
  setEventsHasMore: Dispatch<SetStateAction<boolean>>;
  setSnapshot: Dispatch<SetStateAction<TeamRunSnapshotRecord | null>>;
  setSnapshotLoading: Dispatch<SetStateAction<boolean>>;
  setInbox: (next: TeamActorMessageRecord[]) => void;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setMemberEventsLoading: Dispatch<SetStateAction<boolean>>;
  setMemberEventsHasMore: Dispatch<SetStateAction<boolean>>;
  setActiveRunId: Dispatch<SetStateAction<string | null>>;
  setRunLookupId: (next: string) => void;
  onRunCreated?: (created: TeamRunRecord) => void;
  onTeamsRefreshSettled?: () => void;
};

type TeamApiClient = {
  listAgents: () => Promise<AgentRecord[]>;
  listTeams: () => Promise<TeamDefinitionRecord[]>;
  getTeamRun: (runId: string) => Promise<TeamRunRecord>;
  listTeamRuns: (
    teamId: string,
    payload: {
      limit?: number;
      status?: TeamRunRecord["status"];
      before_created_at?: number;
    }
  ) => Promise<TeamRunRecord[]>;
  listTeamRunSteps: (runId: string) => Promise<TeamStepRecord[]>;
  listTeamRunEvents: (
    runId: string,
    limit: number,
    beforeId?: number
  ) => Promise<TeamRunEventRecord[]>;
  getTeamRunSnapshot: (
    runId: string,
    payload: { event_limit?: number; message_limit?: number }
  ) => Promise<TeamRunSnapshotRecord>;
  listTeamRunInbox: (
    runId: string,
    payload: {
      actor_id: string;
      limit?: number;
      after_id?: number;
      include_delivered?: boolean;
    }
  ) => Promise<TeamActorMessageRecord[]>;
  listAgentEvents: (
    memberAgentId: string,
    limit: number,
    sessionId?: string,
    beforeId?: number
  ) => Promise<AgentEvent[]>;
  createTeamRun: (
    teamId: string,
    payload: { context_id?: string; input?: unknown }
  ) => Promise<TeamRunRecord>;
  cancelTeamRun: (runId: string) => Promise<TeamRunRecord>;
  resumeTeamRun: (runId: string) => Promise<TeamRunRecord>;
  restartTeamRun: (runId: string) => Promise<TeamRunRecord>;
};

function buildTeamApiClient(token: string): TeamApiClient {
  return {
    listAgents: () => api.listAgents(token),
    listTeams: () => api.listTeams(token),
    getTeamRun: (runId) => api.getTeamRun(token, runId),
    listTeamRuns: (teamId, payload) => api.listTeamRuns(token, teamId, payload),
    listTeamRunSteps: (runId) => api.listTeamRunSteps(token, runId),
    listTeamRunEvents: (runId, limit, beforeId) =>
      api.listTeamRunEvents(token, runId, limit, beforeId),
    getTeamRunSnapshot: (runId, payload) => api.getTeamRunSnapshot(token, runId, payload),
    listTeamRunInbox: (runId, payload) => api.listTeamRunInbox(token, runId, payload),
    listAgentEvents: (memberAgentId, limit, sessionId, beforeId) =>
      api.listAgentEvents(token, memberAgentId, limit, sessionId, beforeId),
    createTeamRun: (teamId, payload) => api.createTeamRun(token, teamId, payload),
    cancelTeamRun: (runId) => api.cancelTeamRun(token, runId),
    resumeTeamRun: (runId) => api.resumeTeamRun(token, runId),
    restartTeamRun: (runId) => api.restartTeamRun(token, runId),
  };
}

function hasTeamMismatch(
  run: TeamRunRecord,
  selectedTeamId: string | null,
  actionLabel: string,
  setError: Dispatch<SetStateAction<string | null>>
): boolean {
  if (!selectedTeamId || run.team_id === selectedTeamId) {
    return false;
  }
  setError(
    `Run ${run.id} belongs to team ${run.team_id}. ${actionLabel} applies only to the selected team.`
  );
  return true;
}

export function useTeamActions(options: UseTeamActionsOptions) {
  const {
    token,
    selectedTeamId,
    runContextId,
    runInput,
    runLookupId,
    runStatusFilter,
    runsLoading,
    runsHasMore,
    runsBeforeCreatedAt,
    selectedStepId,
    activeRunIdForSelectedTeam,
    activeRunForSelectedTeam,
    inboxActorId,
    inboxLimit,
    inboxAfterId,
    inboxIncludeDelivered,
    selectedMemberAgentId,
    selectedMemberSessionId,
    selectedMemberSnapshot,
    activeRunIdRef,
    eventsRef,
    memberEventsRef,
    setBusy,
    setError,
    setAgents,
    setTeams,
    setSelectedTeamId,
    setRuns,
    setTeamRunBrowserByTeam,
    setRunsLoading,
    setSteps,
    setSelectedStepId,
    setEvents,
    setEventsLoading,
    setEventsHasMore,
    setSnapshot,
    setSnapshotLoading,
    setInbox,
    setMemberEvents,
    setMemberEventsLoading,
    setMemberEventsHasMore,
    setActiveRunId,
    setRunLookupId,
    onRunCreated,
    onTeamsRefreshSettled,
  } = options;

  const teamApi = useMemo(() => buildTeamApiClient(token), [token]);
  const selectedStepIdRef = useRef(selectedStepId);
  selectedStepIdRef.current = selectedStepId;

  const inboxQueryStateRef = useRef({
    activeRunIdForSelectedTeam,
    inboxActorId,
    inboxLimit,
    inboxAfterId,
    inboxIncludeDelivered,
  });
  inboxQueryStateRef.current = {
    activeRunIdForSelectedTeam,
    inboxActorId,
    inboxLimit,
    inboxAfterId,
    inboxIncludeDelivered,
  };

  const runPaginationStateRef = useRef({
    selectedTeamId,
    runsLoading,
    runsHasMore,
    runsBeforeCreatedAt,
    runStatusFilter,
  });
  runPaginationStateRef.current = {
    selectedTeamId,
    runsLoading,
    runsHasMore,
    runsBeforeCreatedAt,
    runStatusFilter,
  };

  const refreshAgents = useCallback(async () => {
    const list = await teamApi.listAgents();
    setAgents(list);
    return list;
  }, [setAgents, teamApi]);

  const refreshTeams = useCallback(async () => {
    setBusy("refresh-teams");
    setError(null);
    try {
      const list = await teamApi.listTeams();
      setTeams(list);
      setSelectedTeamId((prev) => {
        if (prev && list.some((team) => team.id === prev)) {
          return prev;
        }
        return list[0]?.id ?? null;
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
      onTeamsRefreshSettled?.();
    }
  }, [onTeamsRefreshSettled, setBusy, setError, setSelectedTeamId, setTeams, teamApi]);

  const refreshRun = useCallback(
    async (runId: string) => {
      const run = await teamApi.getTeamRun(runId);
      setRuns((prev) => upsertRun(prev, run));
      return run;
    },
    [setRuns, teamApi]
  );

  const refreshTeamRuns = useCallback(
    async (
      teamId: string,
      mode: "replace" | "append" = "replace",
      options?: {
        statusFilter?: TeamRunStatusFilter;
        beforeCreatedAt?: number;
      }
    ) => {
      setRunsLoading(true);
      try {
        const statusFilter = options?.statusFilter ?? "all";
        const beforeCreatedAt = mode === "append" ? options?.beforeCreatedAt : undefined;
        const list = await teamApi.listTeamRuns(teamId, {
          limit: TEAM_RUN_PAGE_LIMIT,
          before_created_at: beforeCreatedAt,
          status: resolveRunStatusFilter(statusFilter),
        });
        setRuns((prev) => {
          const otherTeamRuns = prev.filter((run) => run.team_id !== teamId);
          const currentTeamRuns = prev.filter((run) => run.team_id === teamId);
          const merged = mergeTeamRunList(
            currentTeamRuns,
            list,
            mode,
            activeRunIdRef.current
          );
          return mergeRunPages(otherTeamRuns, merged);
        });
        const hasMore = list.length >= TEAM_RUN_PAGE_LIMIT;
        const nextBeforeCreatedAt =
          list.length > 0 ? list[list.length - 1]?.created_at : undefined;
        setTeamRunBrowserByTeam((prev) => ({
          ...prev,
          [teamId]: {
            statusFilter,
            hasMore,
            beforeCreatedAt: hasMore ? nextBeforeCreatedAt : undefined,
          },
        }));
        return list;
      } finally {
        setRunsLoading(false);
      }
    },
    [activeRunIdRef, setRuns, setRunsLoading, setTeamRunBrowserByTeam, teamApi]
  );

  const refreshSteps = useCallback(
    async (runId: string) => {
      const list = await teamApi.listTeamRunSteps(runId);
      setSteps(list);
      const currentSelectedStepId = selectedStepIdRef.current;
      const nextSelectedStepId =
        currentSelectedStepId &&
        list.some((step) => step.id === currentSelectedStepId)
          ? currentSelectedStepId
          : list[0]?.id ?? "";
      setSelectedStepId(nextSelectedStepId);
      return list;
    },
    [setSelectedStepId, setSteps, teamApi]
  );

  const refreshEvents = useCallback(
    async (runId: string, mode: "replace" | "prepend" = "replace") => {
      setEventsLoading(true);
      try {
        const beforeId = mode === "prepend" ? eventsRef.current[0]?.event_id : undefined;
        const list = await teamApi.listTeamRunEvents(runId, EVENT_PAGE_LIMIT, beforeId);
        setEvents((prev) => upsertEventList(prev, list, mode));
        setEventsHasMore(list.length >= EVENT_PAGE_LIMIT);
      } finally {
        setEventsLoading(false);
      }
    },
    [eventsRef, setEvents, setEventsHasMore, setEventsLoading, teamApi]
  );

  const refreshSnapshot = useCallback(
    async (runId: string) => {
      setSnapshotLoading(true);
      try {
        const next = await teamApi.getTeamRunSnapshot(runId, {
          event_limit: 200,
          message_limit: 200,
        });
        setSnapshot(next);
        return next;
      } finally {
        setSnapshotLoading(false);
      }
    },
    [setSnapshot, setSnapshotLoading, teamApi]
  );

  const loadInbox = useCallback(
    async (actorIdOverride?: string) => {
      const {
        activeRunIdForSelectedTeam: runId,
        inboxActorId: defaultActorId,
        inboxLimit: currentInboxLimit,
        inboxAfterId: currentInboxAfterId,
        inboxIncludeDelivered: includeDelivered,
      } = inboxQueryStateRef.current;
      if (!runId) return;
      const actorId = (actorIdOverride ?? defaultActorId).trim();
      if (!actorId) {
        throw new Error("Inbox actor_id is required");
      }
      const limit = parseOptionalInteger(currentInboxLimit, "Inbox limit") ?? 100;
      const afterId = parseOptionalInteger(currentInboxAfterId, "Inbox after_id");
      const list = await teamApi.listTeamRunInbox(runId, {
        actor_id: actorId,
        limit,
        after_id: afterId,
        include_delivered: includeDelivered,
      });
      setInbox(list);
    },
    [setInbox, teamApi]
  );

  const loadMemberEvents = useCallback(
    async (mode: "replace" | "prepend" = "replace") => {
      const agentId = selectedMemberAgentId?.trim() ?? "";
      if (!agentId) {
        setMemberEvents([]);
        setMemberEventsHasMore(false);
        return;
      }
      const sessionId =
        selectedMemberSessionId ??
        getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step) ??
        undefined;
      if (!sessionId) {
        setMemberEvents([]);
        setMemberEventsHasMore(false);
        return;
      }

      setMemberEventsLoading(true);
      try {
        const beforeId =
          mode === "prepend" ? memberEventsRef.current[0]?.event_id : undefined;
        const list = await teamApi.listAgentEvents(
          agentId,
          MEMBER_EVENT_PAGE_LIMIT,
          sessionId,
          beforeId
        );
        const currentSessionEvents = memberEventsRef.current.filter(
          (event) => (event.session_id ?? null) === sessionId
        );
        const preserveLoadedHistory =
          mode === "replace" &&
          currentSessionEvents.length > 0 &&
          list.length > 0 &&
          currentSessionEvents[0]!.event_id < list[0]!.event_id;
        setMemberEvents((prev) =>
          upsertAgentEventList(prev, list, mode, sessionId)
        );
        if (!preserveLoadedHistory) {
          setMemberEventsHasMore(list.length >= MEMBER_EVENT_PAGE_LIMIT);
        }
      } finally {
        setMemberEventsLoading(false);
      }
    },
    [
      memberEventsRef,
      selectedMemberAgentId,
      selectedMemberSessionId,
      selectedMemberSnapshot,
      setMemberEvents,
      setMemberEventsHasMore,
      setMemberEventsLoading,
      teamApi,
    ]
  );

  const onCreateRun = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    setBusy("create-run");
    setError(null);
    try {
      const created = await teamApi.createTeamRun(selectedTeamId, {
        context_id: runContextId.trim() || undefined,
        input: parseOptionalJson(runInput, "Run input") ?? {},
      });
      if (onRunCreated) {
        onRunCreated(created);
      } else {
        setRuns((prev) => upsertRun(prev, created));
        setActiveRunId(created.id);
        setRunLookupId(created.id);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    runContextId,
    runInput,
    selectedTeamId,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    teamApi,
    onRunCreated,
  ]);

  const onLoadRunById = useCallback(async () => {
    if (!selectedTeamId) {
      setError("Select a team first");
      return;
    }
    const runId = runLookupId.trim();
    if (!runId) {
      setError("Run ID is required");
      return;
    }
    setBusy("load-run");
    setError(null);
    try {
      const run = await refreshRun(runId);
      if (run.team_id !== selectedTeamId) {
        setError(
          `Run ${run.id} belongs to team ${run.team_id}. Load Run applies only to the selected team.`
        );
        return;
      }
      setActiveRunId(run.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [refreshRun, runLookupId, selectedTeamId, setActiveRunId, setBusy, setError]);

  const onRefreshRuns = useCallback(async () => {
    if (!selectedTeamId) return;
    setError(null);
    try {
      await refreshTeamRuns(selectedTeamId, "replace", {
        statusFilter: runStatusFilter,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [refreshTeamRuns, runStatusFilter, selectedTeamId, setError]);

  const onLoadMoreRuns = useCallback(async () => {
    const {
      selectedTeamId: teamId,
      runsLoading: loading,
      runsHasMore: hasMore,
      runsBeforeCreatedAt: beforeCreatedAt,
      runStatusFilter: statusFilter,
    } = runPaginationStateRef.current;
    if (!teamId || loading || !hasMore) {
      return;
    }
    setError(null);
    try {
      await refreshTeamRuns(teamId, "append", {
        statusFilter,
        beforeCreatedAt,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [refreshTeamRuns, setError]);

  const onCancelRun = useCallback(async () => {
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("cancel-run");
    setError(null);
    try {
      const canceled = await teamApi.cancelTeamRun(runId);
      if (hasTeamMismatch(canceled, selectedTeamId, "Cancel", setError)) {
        return;
      }
      setRuns((prev) => upsertRun(prev, canceled));
      await Promise.all([refreshEvents(runId), refreshSnapshot(runId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunForSelectedTeam,
    refreshEvents,
    refreshSnapshot,
    selectedTeamId,
    setBusy,
    setError,
    setRuns,
    teamApi,
  ]);

  const onResumeRun = useCallback(async () => {
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("resume-run");
    setError(null);
    try {
      const resumed = await teamApi.resumeTeamRun(runId);
      if (hasTeamMismatch(resumed, selectedTeamId, "Resume", setError)) {
        return;
      }
      setRuns((prev) => upsertRun(prev, resumed));
      setActiveRunId(resumed.id);
      setRunLookupId(resumed.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunForSelectedTeam,
    selectedTeamId,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    teamApi,
  ]);

  const onRestartRun = useCallback(async () => {
    if (!activeRunForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    const runId = activeRunForSelectedTeam.id;
    setBusy("restart-run");
    setError(null);
    try {
      const restarted = await teamApi.restartTeamRun(runId);
      if (hasTeamMismatch(restarted, selectedTeamId, "Restart", setError)) {
        return;
      }
      setRuns((prev) => upsertRun(prev, restarted));
      setActiveRunId(restarted.id);
      setRunLookupId(restarted.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunForSelectedTeam,
    selectedTeamId,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    teamApi,
  ]);

  return {
    refreshAgents,
    refreshTeams,
    refreshRun,
    refreshTeamRuns,
    refreshSteps,
    refreshEvents,
    refreshSnapshot,
    loadInbox,
    loadMemberEvents,
    onCreateRun,
    onLoadRunById,
    onRefreshRuns,
    onLoadMoreRuns,
    onCancelRun,
    onResumeRun,
    onRestartRun,
  };
}
