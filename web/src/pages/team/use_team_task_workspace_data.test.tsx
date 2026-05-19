// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, type TeamTaskDetailResponse } from "../../api";
import { useTeamTaskWorkspaceData } from "./use_team_task_workspace_data";

const { getApiErrorStatusMock } = vi.hoisted(() => ({
  getApiErrorStatusMock: vi.fn<() => number | null>(() => null),
}));

vi.mock("../../api", () => ({
  api: {
    listTeamTasks: vi.fn(),
    getTeamSharedThread: vi.fn(),
    getTeamTask: vi.fn(),
  },
  getApiErrorStatus: getApiErrorStatusMock,
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamTaskWorkspaceData>[0];
type HookSnapshot = ReturnType<typeof useTeamTaskWorkspaceData>;

function HookHarness({
  params,
  onCapture,
}: {
  params: HookParams;
  onCapture: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamTaskWorkspaceData(params);
  useEffect(() => {
    onCapture(snapshot);
  }, [onCapture, snapshot]);
  return null;
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    effectiveSelectedTeamId: "team-1",
    routeChannelId: "all",
    routeSelectedTaskId: "",
    selectedChannelTaskId: "shared-thread",
    selectedConversationTaskId: "",
    selectedConversationDetail: null,
    selectedTaskDetail: null,
    sharedConversation: null,
    sharedConversationLatestRun: null,
    taskList: [],
    tasksLoading: false,
    selectedTaskId: "",
    sharedConversationRequestScopeRef: {
      current: {
        teamId: "team-1",
        requestSeq: 1,
      },
    },
    setError: vi.fn(),
    setTaskList: vi.fn(),
    setSharedConversation: vi.fn(),
    setSharedConversationLatestRun: vi.fn(),
    setSelectedConversationDetail: vi.fn(),
    setSelectedTaskDetail: vi.fn(),
    setTasksLoading: vi.fn(),
    setSelectedTaskId: vi.fn(),
    setTaskMessages: vi.fn(),
    setConversationMailboxMessages: vi.fn(),
    setSelectedConversationTaskId: vi.fn(),
    setCompiledRunPreview: vi.fn(),
    setCompilePreviewContextId: vi.fn(),
    ...overrides,
  };
}

async function mountHook(params: HookParams) {
  let snapshot: HookSnapshot | null = null;
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(
      <HookHarness
        params={params}
        onCapture={(next) => {
          snapshot = next;
        }}
      />
    );
    await Promise.resolve();
  });
  return {
    getSnapshot: () => snapshot,
    rerender: async (nextParams: HookParams) => {
      await act(async () => {
        root.render(
          <HookHarness
            params={nextParams}
            onCapture={(next) => {
              snapshot = next;
            }}
          />
        );
        await Promise.resolve();
        await Promise.resolve();
      });
    },
    cleanup: () => {
      act(() => {
        root.unmount();
      });
      container.remove();
    },
  };
}

describe("useTeamTaskWorkspaceData", () => {
  const mockedApi = vi.mocked(api);

  afterEach(() => {
    vi.clearAllMocks();
    getApiErrorStatusMock.mockReset();
    getApiErrorStatusMock.mockReturnValue(null);
  });

  it("loads task list and shared conversation when a team becomes active", async () => {
    mockedApi.listTeamTasks.mockResolvedValueOnce([
      {
        id: "task-1",
        team_id: "team-1",
        title: "Implement feature",
        status: "working",
        created_by_actor_id: "coordinator",
        assigned_member_id: null,
        context: {},
        created_at: 1,
        updated_at: 5,
      },
    ] as never);
    mockedApi.getTeamSharedThread.mockResolvedValueOnce({
      task: {
        id: "shared-thread",
        team_id: "team-1",
        title: "all",
        status: "working",
        created_by_actor_id: "coordinator",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 6,
      },
      latest_run: null,
    } as never);

    const params = createParams();
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await Promise.resolve();
      });

      expect(mockedApi.listTeamTasks).toHaveBeenCalledWith("token-1", "team-1", 100, {
        include_shared_thread: true,
      });
      expect(mockedApi.getTeamSharedThread).toHaveBeenCalledWith("token-1", "team-1");
      expect(params.setTasksLoading).toHaveBeenNthCalledWith(1, true);
      expect(params.setTaskList).toHaveBeenCalled();
      expect(params.setSharedConversation).toHaveBeenCalled();
      expect(params.setCompiledRunPreview).toHaveBeenCalledWith(null);
      expect(params.setCompilePreviewContextId).toHaveBeenCalledWith("");
      expect(mounted.getSnapshot()?.resolvedSelectedConversationTaskId).toBe("");
    } finally {
      mounted.cleanup();
    }
  });

  it("keeps the channel lane pinned to the shared thread when stale task selection remains", async () => {
    const sharedConversation = {
      id: "shared-thread",
      team_id: "team-1",
      title: "all",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { bootstrap_kind: "shared_thread" },
      created_at: 1,
      updated_at: 10,
    } as const;
    const staleTask = {
      id: "task-2",
      team_id: "team-1",
      title: "Implementation thread",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { bootstrap_kind: "team_channel", channel_id: "review" },
      created_at: 2,
      updated_at: 20,
    } as const;
    const staleDetail: NonNullable<HookParams["selectedConversationDetail"]> = {
      task: staleTask,
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "review",
        created_at: 2,
        updated_at: 2,
      },
      latest_run: {
        id: "run-task-2",
        team_id: "team-1",
        context_id: "",
        status: "working",
        input: {},
        summary: null,
        created_at: 2,
        started_at: 2,
        ended_at: null,
      },
    };
    const sharedLatestRun = {
      id: "run-shared",
      team_id: "team-1",
      context_id: "",
      status: "working",
      input: {},
      summary: null,
      created_at: 1,
      started_at: 1,
      ended_at: null,
    } as unknown as HookParams["sharedConversationLatestRun"];
    mockedApi.getTeamTask.mockResolvedValue(staleDetail);

    const mounted = await mountHook(
      createParams({
        routeChannelId: "all",
        selectedChannelTaskId: "shared-thread",
        selectedConversationTaskId: "task-2",
        sharedConversation,
        sharedConversationLatestRun: sharedLatestRun,
        selectedConversationDetail: staleDetail,
        taskList: [sharedConversation, staleTask],
      })
    );
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.selectedConversation?.id).toBe("shared-thread");
      expect(snapshot?.selectedConversationLatestRun?.id).toBe("run-shared");
    } finally {
      mounted.cleanup();
    }
  });

  it("preserves an explicit task route on the shared channel lane", async () => {
    const sharedConversation = {
      id: "shared-thread",
      team_id: "team-1",
      title: "all",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { bootstrap_kind: "shared_thread" },
      created_at: 1,
      updated_at: 10,
    } as const;
    const explicitTask = {
      id: "task-2",
      team_id: "team-1",
      title: "Implementation thread",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { owner: "coordinator" },
      created_at: 2,
      updated_at: 20,
    } as const;
    const explicitDetail: NonNullable<HookParams["selectedConversationDetail"]> = {
      task: explicitTask,
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "implementation",
        created_at: 2,
        updated_at: 2,
      },
      latest_run: null,
    };

    const mounted = await mountHook(
      createParams({
        routeChannelId: "all",
        routeSelectedTaskId: "task-2",
        selectedChannelTaskId: "shared-thread",
        selectedConversationTaskId: "task-2",
        sharedConversation,
        selectedConversationDetail: explicitDetail,
        taskList: [sharedConversation, explicitTask],
      })
    );
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.selectedConversation?.id).toBe("task-2");
    } finally {
      mounted.cleanup();
    }
  });

  it("preserves an explicit task route on a named channel lane", async () => {
    const explicitTask = {
      id: "task-2",
      team_id: "team-1",
      title: "Implementation thread",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { owner: "coordinator" },
      created_at: 2,
      updated_at: 20,
    } as const;
    const explicitDetail: NonNullable<HookParams["selectedConversationDetail"]> = {
      task: explicitTask,
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "implementation",
        created_at: 2,
        updated_at: 2,
      },
      latest_run: null,
    };
    const channelTask = {
      id: "task-review",
      team_id: "team-1",
      title: "review",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { bootstrap_kind: "team_channel", channel_id: "review" },
      created_at: 1,
      updated_at: 10,
    } as const;

    const mounted = await mountHook(
      createParams({
        routeChannelId: "review",
        routeSelectedTaskId: "task-2",
        selectedChannelTaskId: "task-review",
        selectedConversationTaskId: "task-2",
        selectedConversationDetail: explicitDetail,
        taskList: [channelTask, explicitTask],
      })
    );
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.selectedConversation?.id).toBe("task-2");
    } finally {
      mounted.cleanup();
    }
  });

  it("uses the selected conversation detail run when the explicit task thread is active", async () => {
    const explicitTask = {
      id: "task-2",
      team_id: "team-1",
      title: "Implementation thread",
      status: "in_progress",
      created_by_actor_id: "coordinator",
      assigned_member_id: null,
      context: { owner: "coordinator" },
      created_at: 2,
      updated_at: 20,
    } as const;
    const explicitLatestRun = {
      id: "run-task-2",
      team_id: "team-1",
      context_id: "",
      status: "working",
      input: {},
      summary: null,
      created_at: 2,
      started_at: 2,
      ended_at: null,
    } as unknown as HookParams["sharedConversationLatestRun"];
    const explicitDetail: NonNullable<HookParams["selectedConversationDetail"]> = {
      task: explicitTask,
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "implementation",
        created_at: 2,
        updated_at: 2,
      },
      latest_run: explicitLatestRun,
    };

    const mounted = await mountHook(
      createParams({
        routeChannelId: "all",
        routeSelectedTaskId: "task-2",
        selectedChannelTaskId: "shared-thread",
        selectedConversationTaskId: "task-2",
        selectedConversationDetail: explicitDetail,
        taskList: [explicitTask],
      })
    );
    try {
      const snapshot = mounted.getSnapshot();
      expect(snapshot?.selectedConversation?.id).toBe("task-2");
      expect(snapshot?.selectedConversationLatestRun?.id).toBe("run-task-2");
    } finally {
      mounted.cleanup();
    }
  });

  it("clears a selected conversation only after the detail fetch confirms a 404", async () => {
    mockedApi.listTeamTasks.mockResolvedValue([] as never);
    mockedApi.getTeamSharedThread.mockResolvedValue({
      task: {
        id: "shared-thread",
        team_id: "team-1",
        title: "all",
        status: "working",
        created_by_actor_id: "coordinator",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 6,
      },
      latest_run: null,
    } as never);
    mockedApi.getTeamTask.mockRejectedValueOnce({
      status: 404,
      message: "task not found",
    } as never);
    getApiErrorStatusMock.mockReturnValue(404);

    const params = createParams({
      selectedConversationTaskId: "task-2",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(mockedApi.getTeamTask).toHaveBeenCalledWith("token-1", "team-1", "task-2");
      expect(params.setSelectedConversationDetail).toHaveBeenCalledWith(null);
      expect(params.setSelectedConversationTaskId).toHaveBeenCalledWith("");
    } finally {
      mounted.cleanup();
    }
  });

  it("keeps the selected conversation when the detail fetch fails without a 404", async () => {
    mockedApi.listTeamTasks.mockResolvedValue([] as never);
    mockedApi.getTeamSharedThread.mockResolvedValue({
      task: {
        id: "shared-thread",
        team_id: "team-1",
        title: "all",
        status: "working",
        created_by_actor_id: "coordinator",
        assigned_member_id: null,
        context: { bootstrap_kind: "shared_thread" },
        created_at: 1,
        updated_at: 6,
      },
      latest_run: null,
    } as never);
    mockedApi.getTeamTask.mockRejectedValueOnce(new Error("network failed"));
    getApiErrorStatusMock.mockReturnValue(null);

    const params = createParams({
      selectedConversationTaskId: "task-2",
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(mockedApi.getTeamTask).toHaveBeenCalledWith("token-1", "team-1", "task-2");
      expect(params.setSelectedConversationTaskId).not.toHaveBeenCalledWith("");
    } finally {
      mounted.cleanup();
    }
  });

  it("loads selected task detail for the task lane and reuses an existing snapshot", async () => {
    const detail: TeamTaskDetailResponse = {
      task: {
        id: "task-2",
        team_id: "team-1",
        title: "Prepare rollout",
        status: "in_progress",
        priority: "critical",
        created_by_actor_id: "coordinator",
        assigned_member_id: "worker-1",
        context: { owner: "coordinator" },
        created_at: 2,
        updated_at: 3,
      },
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "review",
        created_at: 2,
        updated_at: 3,
      },
      latest_run: null,
      notes: [],
    };
    mockedApi.getTeamTask.mockResolvedValueOnce(detail as never);

    const params = createParams({
      selectedTaskId: "task-2",
      taskList: [detail.task],
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(mockedApi.getTeamTask).toHaveBeenCalledWith("token-1", "team-1", "task-2");
      expect(params.setSelectedTaskDetail).toHaveBeenCalledWith(detail);

      mockedApi.getTeamTask.mockClear();
      await mounted.rerender(
        createParams({
          selectedTaskId: "task-2",
          taskList: [detail.task],
          selectedTaskDetail: detail,
          setSelectedTaskDetail: params.setSelectedTaskDetail,
        })
      );
      expect(mockedApi.getTeamTask).not.toHaveBeenCalled();
    } finally {
      mounted.cleanup();
    }
  });

  it("clears the selected task detail when task selection resets", async () => {
    const selectedTaskDetail: TeamTaskDetailResponse = {
      task: {
        id: "task-2",
        team_id: "team-1",
        title: "Prepare rollout",
        status: "in_progress",
        priority: "medium",
        created_by_actor_id: "coordinator",
        assigned_member_id: "worker-1",
        context: {},
        created_at: 2,
        updated_at: 3,
      },
      conversation: {
        id: "conv-task-2",
        team_id: "team-1",
        task_id: "task-2",
        mode: "group_chat",
        topic: "review",
        created_at: 2,
        updated_at: 3,
      },
      latest_run: null,
      notes: [],
    };
    const params = createParams({
      selectedTaskId: "task-2",
      selectedTaskDetail,
    });
    const mounted = await mountHook(params);
    try {
      await mounted.rerender(
        createParams({
          selectedTaskId: "",
          selectedTaskDetail,
          setSelectedTaskDetail: params.setSelectedTaskDetail,
        })
      );
      expect(params.setSelectedTaskDetail).toHaveBeenCalledWith(null);
    } finally {
      mounted.cleanup();
    }
  });
});
