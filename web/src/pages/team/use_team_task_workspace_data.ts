import { useCallback, useEffect, useMemo, useState, type Dispatch, type MutableRefObject, type SetStateAction } from "react";
import {
  api,
  getApiErrorStatus,
  type TeamConversationMessageRecord,
  type TeamRunRecord,
  type TeamTaskDetailResponse,
  type TeamTaskRecord,
} from "../../api";
import { parseErrorMessage } from "./create_helpers";
import {
  isCurrentTeamScopedRequest,
  listTeamWorkspaceTasks,
  resolveChannelLaneConversationTask,
  resolveSelectedConversationLatestRun,
  resolveSelectedConversationTask,
  resolveSelectedTeamTask,
  shouldClearSelectedConversationTask,
  sortTasksByActivity,
} from "./page_helpers";

type UseTeamTaskWorkspaceDataOptions = {
  token: string;
  effectiveSelectedTeamId: string | null;
  routeChannelId: string;
  routeSelectedTaskId?: string | null;
  selectedChannelTaskId?: string | null;
  selectedConversationTaskId: string;
  selectedConversationDetail: TeamTaskDetailResponse | null;
  selectedTaskDetail: TeamTaskDetailResponse | null;
  sharedConversation: TeamTaskRecord | null;
  sharedConversationLatestRun: TeamRunRecord | null;
  taskList: TeamTaskRecord[];
  tasksLoading: boolean;
  selectedTaskId: string;
  sharedConversationRequestScopeRef: MutableRefObject<{ teamId: string; requestSeq: number }>;
  setError: Dispatch<SetStateAction<string | null>>;
  setTaskList: Dispatch<SetStateAction<TeamTaskRecord[]>>;
  setSharedConversation: Dispatch<SetStateAction<TeamTaskRecord | null>>;
  setSharedConversationLatestRun: Dispatch<SetStateAction<TeamRunRecord | null>>;
  setSelectedConversationDetail: Dispatch<SetStateAction<TeamTaskDetailResponse | null>>;
  setSelectedTaskDetail: Dispatch<SetStateAction<TeamTaskDetailResponse | null>>;
  setTasksLoading: Dispatch<SetStateAction<boolean>>;
  setSelectedTaskId: Dispatch<SetStateAction<string>>;
  setTaskMessages: Dispatch<SetStateAction<TeamConversationMessageRecord[]>>;
  setConversationMailboxMessages: Dispatch<SetStateAction<import("../../api").TeamActorMessageRecord[]>>;
  setSelectedConversationTaskId: Dispatch<SetStateAction<string>>;
  setCompiledRunPreview: Dispatch<SetStateAction<import("../../api").TeamTaskRunCompilePreviewRecord | null>>;
  setCompilePreviewContextId: Dispatch<SetStateAction<string>>;
};

export function useTeamTaskWorkspaceData(options: UseTeamTaskWorkspaceDataOptions) {
  const {
    token,
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    selectedChannelTaskId,
    selectedConversationTaskId,
    selectedConversationDetail,
    selectedTaskDetail,
    sharedConversation,
    sharedConversationLatestRun,
    taskList,
    tasksLoading,
    selectedTaskId,
    sharedConversationRequestScopeRef,
    setError,
    setTaskList,
    setSharedConversation,
    setSharedConversationLatestRun,
    setSelectedConversationDetail,
    setSelectedTaskDetail,
    setTasksLoading,
    setSelectedTaskId,
    setTaskMessages,
    setConversationMailboxMessages,
    setSelectedConversationTaskId,
    setCompiledRunPreview,
    setCompilePreviewContextId,
  } = options;

  const resolvedSelectedConversationTaskId = (routeSelectedTaskId?.trim() || selectedConversationTaskId.trim()).trim();
  const [selectedConversationDetailMissing, setSelectedConversationDetailMissing] = useState(false);

  const selectedConversation = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return null;
    }
    const resolvedConversation = resolveSelectedConversationTask({
      taskList,
      selectedTaskId: resolvedSelectedConversationTaskId,
      sharedConversation,
      fallbackTask: selectedConversationDetail?.task ?? null,
    });
    const nextConversation = resolveChannelLaneConversationTask({
      routeChannelId,
      routeSelectedTaskId,
      selectedConversationTaskId: resolvedSelectedConversationTaskId,
      selectedConversation: resolvedConversation,
      selectedChannelTaskId,
      sharedConversation,
      taskList,
    });
    return nextConversation;
  }, [
    effectiveSelectedTeamId,
    routeChannelId,
    routeSelectedTaskId,
    resolvedSelectedConversationTaskId,
    selectedChannelTaskId,
    selectedConversationDetail?.task,
    sharedConversation,
    taskList,
  ]);

  const selectedConversationLatestRun = useMemo(() => {
    return resolveSelectedConversationLatestRun({
      selectedConversation,
      selectedConversationDetail,
      sharedConversation,
      sharedConversationLatestRun,
    });
  }, [
    selectedConversation,
    selectedConversationDetail,
    sharedConversation,
    sharedConversationLatestRun,
  ]);

  const selectedConversationId = selectedConversation?.id ?? null;

  const workspaceTasks = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return [];
    }
    return listTeamWorkspaceTasks(taskList, effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, taskList]);

  const selectedTask = useMemo(() => {
    if (!effectiveSelectedTeamId) {
      return null;
    }
    return resolveSelectedTeamTask(taskList, selectedTaskId, effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, selectedTaskId, taskList]);

  const refreshTasks = useCallback(
    async (teamId: string) => {
      setTasksLoading(true);
      try {
        const list = await api.listTeamTasks(token, teamId, 100, {
          include_shared_thread: true,
        });
        const sorted = sortTasksByActivity(list);
        setTaskList(sorted);
        setSelectedTaskId((prev) => resolveSelectedTeamTask(sorted, prev, teamId)?.id ?? "");
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setTasksLoading(false);
      }
    },
    [setError, setSelectedTaskId, setTaskList, setTasksLoading, token]
  );

  const refreshSharedConversation = useCallback(
    async (teamId: string) => {
      const normalizedTeamId = teamId.trim();
      const requestSeq = sharedConversationRequestScopeRef.current.requestSeq;
      const isCurrentRequest = () =>
        isCurrentTeamScopedRequest(
          sharedConversationRequestScopeRef.current,
          normalizedTeamId,
          requestSeq
        );
      try {
        const detail = await api.getTeamSharedThread(token, normalizedTeamId);
        if (!isCurrentRequest()) {
          return;
        }
        setSharedConversation(detail.task);
        setSharedConversationLatestRun(detail.latest_run ?? null);
      } catch (err) {
        if (!isCurrentRequest()) {
          return;
        }
        if (getApiErrorStatus(err) === 404) {
          setSharedConversation(null);
          setSharedConversationLatestRun(null);
          setTaskMessages([]);
          setConversationMailboxMessages([]);
          return;
        }
        setError(parseErrorMessage(err));
      }
    },
    [
      setConversationMailboxMessages,
      setError,
      setSharedConversation,
      setSharedConversationLatestRun,
      setTaskMessages,
      sharedConversationRequestScopeRef,
      token,
    ]
  );

  const onRefreshTasks = useCallback(async () => {
    if (!effectiveSelectedTeamId) {
      setError("Select a team first");
      return;
    }
    setError(null);
    await refreshTasks(effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, refreshTasks, setError]);

  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      return;
    }
    const taskId = resolvedSelectedConversationTaskId;
    if (!taskId) {
      setSelectedConversationDetailMissing(false);
      setSelectedConversationDetail(null);
      return;
    }
    if (sharedConversation?.id === taskId) {
      setSelectedConversationDetailMissing(false);
      setSelectedConversationDetail(null);
      return;
    }
    let active = true;
    void api
      .getTeamTask(token, effectiveSelectedTeamId, taskId)
      .then((detail) => {
        if (!active) {
          return;
        }
        setSelectedConversationDetailMissing(false);
        setSelectedConversationDetail(detail);
      })
      .catch((err) => {
        if (!active) {
          return;
        }
        setSelectedConversationDetailMissing(getApiErrorStatus(err) === 404);
        setSelectedConversationDetail(null);
        setError(parseErrorMessage(err));
      });
    return () => {
      active = false;
    };
  }, [
    effectiveSelectedTeamId,
    resolvedSelectedConversationTaskId,
    setError,
    setSelectedConversationDetail,
    sharedConversation?.id,
    token,
  ]);

  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      setSelectedTaskDetail(null);
      return;
    }
    const taskId = selectedTaskId.trim();
    if (!taskId) {
      setSelectedTaskDetail(null);
      return;
    }
    if (selectedTaskDetail?.task.id === taskId) {
      return;
    }
    let active = true;
    void api
      .getTeamTask(token, effectiveSelectedTeamId, taskId)
      .then((detail) => {
        if (!active) {
          return;
        }
        setSelectedTaskDetail(detail);
      })
      .catch((err) => {
        if (!active) {
          return;
        }
        setSelectedTaskDetail(null);
        setError(parseErrorMessage(err));
      });
    return () => {
      active = false;
    };
  }, [
    effectiveSelectedTeamId,
    selectedTaskDetail?.task.id,
    selectedTaskId,
    setError,
    setSelectedTaskDetail,
    token,
  ]);

  useEffect(() => {
    const shouldClearSelection = shouldClearSelectedConversationTask({
      selectedConversationTaskId: resolvedSelectedConversationTaskId,
      sharedConversationTaskId: sharedConversation?.id ?? null,
      selectedConversationDetailPresent: Boolean(selectedConversationDetail),
      selectedConversationDetailMissing,
      tasksLoading,
    });
    if (!shouldClearSelection) {
      return;
    }
    setSelectedConversationTaskId("");
    setSelectedConversationDetail(null);
  }, [
    resolvedSelectedConversationTaskId,
    selectedConversationDetail,
    setSelectedConversationDetail,
    setSelectedConversationTaskId,
    selectedConversationDetailMissing,
    sharedConversation?.id,
    tasksLoading,
  ]);

  useEffect(() => {
    setCompiledRunPreview(null);
    setCompilePreviewContextId("");
    setSelectedTaskDetail(null);
  }, [
    effectiveSelectedTeamId,
    selectedTaskId,
    setCompiledRunPreview,
    setCompilePreviewContextId,
    setSelectedTaskDetail,
  ]);

  useEffect(() => {
    if (!effectiveSelectedTeamId) {
      return;
    }
    void refreshTasks(effectiveSelectedTeamId);
    void refreshSharedConversation(effectiveSelectedTeamId);
  }, [effectiveSelectedTeamId, refreshSharedConversation, refreshTasks]);

  return {
    resolvedSelectedConversationTaskId,
    selectedConversation,
    selectedConversationLatestRun,
    selectedConversationId,
    workspaceTasks,
    selectedTask,
    refreshTasks,
    refreshSharedConversation,
    onRefreshTasks,
    selectedConversationDetailMissing,
  };
}
