// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
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
    selectedConversationTaskId: "",
    selectedConversationDetail: null,
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
        created_by_actor_id: "leader",
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
        created_by_actor_id: "leader",
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

  it("clears a selected conversation only after the detail fetch confirms a 404", async () => {
    mockedApi.listTeamTasks.mockResolvedValue([] as never);
    mockedApi.getTeamSharedThread.mockResolvedValue({
      task: {
        id: "shared-thread",
        team_id: "team-1",
        title: "all",
        status: "working",
        created_by_actor_id: "leader",
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
        created_by_actor_id: "leader",
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
});
