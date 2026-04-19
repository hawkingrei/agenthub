// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../api";
import { useTeamTaskWorkspaceData } from "./use_team_task_workspace_data";

vi.mock("../../api", () => ({
  api: {
    listTeamTasks: vi.fn(),
    getTeamSharedThread: vi.fn(),
    getTeamTask: vi.fn(),
  },
  getApiErrorStatus: vi.fn(() => null),
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
  });

  it("loads task list and shared conversation when a team becomes active", async () => {
    mockedApi.listTeamTasks.mockResolvedValueOnce([
      {
        id: "task-1",
        team_id: "team-1",
        title: "Implement feature",
        status: "working",
        kind: "task",
        latest_message_at: 5,
        summary: null,
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
        kind: "shared_thread",
        latest_message_at: 6,
        summary: null,
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

  it("fetches selected conversation detail and clears stale selection when task disappears", async () => {
    mockedApi.listTeamTasks.mockResolvedValue([] as never);
    mockedApi.getTeamSharedThread.mockResolvedValue({
      task: {
        id: "shared-thread",
        team_id: "team-1",
        title: "all",
        status: "working",
        kind: "shared_thread",
        latest_message_at: 6,
        summary: null,
        created_at: 1,
        updated_at: 6,
      },
      latest_run: null,
    } as never);
    mockedApi.getTeamTask.mockResolvedValueOnce({
      task: {
        id: "task-2",
        team_id: "team-1",
        title: "Investigate",
        status: "working",
        kind: "task",
        latest_message_at: 7,
        summary: null,
        created_at: 1,
        updated_at: 7,
      },
      latest_run: {
        id: "run-2",
        team_id: "team-1",
        context_id: "ctx-2",
        status: "working",
        input: {},
        created_at: 1,
        started_at: null,
        ended_at: null,
      },
      messages: [],
      mailbox_messages: [],
    } as never);

    const params = createParams({
      selectedConversationTaskId: "task-2",
      taskList: [
        {
          id: "task-2",
          team_id: "team-1",
          title: "Investigate",
          status: "working",
          kind: "task",
          latest_message_at: 7,
          summary: null,
          created_at: 1,
          updated_at: 7,
        },
      ] as HookParams["taskList"],
    });
    const mounted = await mountHook(params);
    try {
      await act(async () => {
        await Promise.resolve();
      });
      expect(mockedApi.getTeamTask).toHaveBeenCalledWith("token-1", "team-1", "task-2");
      expect(params.setSelectedConversationDetail).toHaveBeenCalled();

      const nextParams = createParams({
        ...params,
        taskList: [
          {
            id: "task-3",
            team_id: "team-1",
            title: "Still present",
            status: "working",
            kind: "task",
            latest_message_at: 8,
            summary: null,
            created_at: 1,
            updated_at: 8,
          },
        ] as HookParams["taskList"],
        selectedConversationDetail: null,
        tasksLoading: false,
      });
      await mounted.rerender(nextParams);
      expect(nextParams.setSelectedConversationTaskId).toHaveBeenCalledWith("");
      expect(nextParams.setSelectedConversationDetail).toHaveBeenCalledWith(null);
    } finally {
      mounted.cleanup();
    }
  });
});
