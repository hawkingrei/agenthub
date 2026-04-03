// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  type TeamActorMessageRecord,
  type TeamConversationMessageRecord,
  type TeamRunRecord,
  type TeamTaskRecord,
  api,
} from "../../api";
import { useTeamConversationActions } from "./use_team_conversation_actions";

vi.mock("../../api", async () => {
  const actual = await vi.importActual<typeof import("../../api")>("../../api");
  return {
    ...actual,
    api: {
      ...actual.api,
      listTeamTaskMessages: vi.fn(),
      getTeamTask: vi.fn(),
      getTeamRunSnapshot: vi.fn(),
      ensureTeamSharedThread: vi.fn(),
      sendTeamTaskMessage: vi.fn(),
    },
  };
});

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type TeamConversationActions = ReturnType<typeof useTeamConversationActions>;
type TeamConversationOptions = Parameters<typeof useTeamConversationActions>[0];

function buildTaskMessage(
  messageId: number,
  text: string
): TeamConversationMessageRecord {
  return {
    message_id: messageId,
    conversation_id: "conv-all",
    task_id: "task-all",
    from_actor_id: "leader",
    to_actor_id: null,
    route: "group_chat",
    payload: { type: "chat_message", text },
    created_at: 1_700_000_000 + messageId,
  };
}

function buildMailboxMessage(messageId: number, text: string): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: "leader",
    to_actor_id: "worker",
    channel: "default",
    payload: { type: "chat_message", text },
    transport: "local",
    status: "pending",
    created_at: 1_700_000_000 + messageId,
    delivered_at: null,
  };
}

function buildSharedThreadTask(): TeamTaskRecord {
  return {
    id: "task-all",
    team_id: "team-1",
    title: "all",
    status: "in_progress",
    created_by_actor_id: "leader",
    assigned_member_id: null,
    context: { bootstrap_kind: "shared_thread" },
    created_at: 1,
    updated_at: 1,
  };
}

function buildRun(id: string): TeamRunRecord {
  return {
    id,
    team_id: "team-1",
    context_id: `ctx-${id}`,
    status: "working",
    input: {},
    created_at: 1,
    started_at: null,
    ended_at: null,
  };
}

function createStateSetter<T>(initial: T) {
  const state = { current: initial };
  const setter = vi.fn((update: React.SetStateAction<T>) => {
    state.current =
      typeof update === "function"
        ? (update as (previous: T) => T)(state.current)
        : update;
  });
  return { state, setter };
}

function createOptions(
  overrides: Partial<TeamConversationOptions> = {}
): TeamConversationOptions {
  const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
  const mailboxMessages = createStateSetter<TeamActorMessageRecord[]>([]);
  return {
    token: "token-1",
    selectedTeamId: "team-1",
    selectedConversation: buildSharedThreadTask(),
    taskMessages: taskMessages.state.current,
    latestRunForSharedConversation: null,
    activeRunIdForSelectedTeam: "run-1",
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    setBusy: vi.fn(),
    setError: vi.fn(),
    setWarning: vi.fn(),
    setSharedConversation: vi.fn(),
    setSharedConversationLatestRun: vi.fn(),
    setTaskMessages: taskMessages.setter,
    setTaskMessagesLoading: vi.fn(),
    setConversationMailboxMessages: mailboxMessages.setter,
    setTaskMessageDraft: vi.fn(),
    ...overrides,
  };
}

function HookHarness({
  options,
  onCapture,
}: {
  options: TeamConversationOptions;
  onCapture: (actions: TeamConversationActions) => void;
}) {
  const actions = useTeamConversationActions(options);
  useEffect(() => {
    onCapture(actions);
  }, [actions, onCapture]);
  return null;
}

async function mountHarness(
  options: TeamConversationOptions,
  onCapture: (actions: TeamConversationActions) => void
): Promise<{ root: Root; container: HTMLDivElement }> {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(<HookHarness options={options} onCapture={onCapture} />);
    await Promise.resolve();
  });
  return { root, container };
}

function cleanupHarness(root: Root, container: HTMLDivElement): void {
  act(() => {
    root.unmount();
  });
  container.remove();
}

describe("useTeamConversationActions", () => {
  const mockedApi = vi.mocked(api);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("loads only the recent shared-thread tail window", async () => {
    mockedApi.listTeamTaskMessages.mockResolvedValueOnce([
      buildTaskMessage(22, "tail-1"),
      buildTaskMessage(41, "tail-2"),
    ]);
    mockedApi.getTeamTask.mockResolvedValue({
      task: buildSharedThreadTask(),
      conversation: {
        id: "conv-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: {
        id: "run-1",
        team_id: "team-1",
        context_id: "ctx-1",
        status: "working",
        input: {},
        created_at: 1,
        started_at: null,
        ended_at: null,
      },
    });
    mockedApi.getTeamRunSnapshot.mockResolvedValueOnce({
      run: {
        id: "run-1",
        team_id: "team-1",
        context_id: "ctx-1",
        status: "working",
        input: {},
        created_at: 1,
        started_at: null,
        ended_at: null,
      },
      team: {
        id: "team-1",
        name: "Team One",
        description: null,
        spec: {},
        created_at: 1,
        updated_at: 1,
      },
      leader_member_id: "leader",
      members: [],
      steps: [],
      latest_events: [],
      mailbox: {
        pending: 0,
        delivered: 0,
        dead_letter: 0,
        recent_messages: [buildMailboxMessage(90, "latest mailbox")],
      },
    } as Awaited<ReturnType<typeof api.getTeamRunSnapshot>>);

    let captured: TeamConversationActions | null = null;
    const options = createOptions();
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      expect(captured).not.toBeNull();
      await act(async () => {
        await captured?.refreshTaskMessages();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(mockedApi.listTeamTaskMessages).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "team-1",
        "task-all",
        { limit: 20 }
      );
      expect(mockedApi.getTeamRunSnapshot).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "run-1",
        { event_limit: 1, message_limit: 20 }
      );
      expect(mockedApi.listTeamTaskMessages).toHaveBeenCalledTimes(1);
      expect(mockedApi.getTeamRunSnapshot).toHaveBeenCalledTimes(1);
      expect(options.setTaskMessagesLoading).toHaveBeenCalledWith(true);
      expect(options.setTaskMessagesLoading).toHaveBeenCalledWith(false);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("loads older shared-thread history with before_id pagination", async () => {
    mockedApi.listTeamTaskMessages.mockResolvedValueOnce([
      buildTaskMessage(5, "older-1"),
      buildTaskMessage(9, "older-2"),
    ]);

    let captured: TeamConversationActions | null = null;
    const options = createOptions({
      taskMessages: [buildTaskMessage(22, "tail-1"), buildTaskMessage(41, "tail-2")],
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.loadOlderTaskMessages();
        await Promise.resolve();
      });

      expect(mockedApi.listTeamTaskMessages).toHaveBeenCalledWith(
        "token-1",
        "team-1",
        "task-all",
        { limit: 20, before_id: 22 }
      );
      expect(options.setTaskMessages).toHaveBeenCalled();
      const finalTaskMessages = (
        options.setTaskMessages as ReturnType<typeof vi.fn>
      ).mock.calls.at(-1)?.[0] as React.SetStateAction<TeamConversationMessageRecord[]>;
      const resolvedMessages =
        typeof finalTaskMessages === "function"
          ? finalTaskMessages([buildTaskMessage(22, "tail-1"), buildTaskMessage(41, "tail-2")])
          : finalTaskMessages;
      expect(resolvedMessages.map((message) => message.message_id)).toEqual([5, 9, 22, 41]);
      expect(captured?.taskMessagesHasMore).toBe(false);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("lets the backend infer direct routing from a single mention", async () => {
    mockedApi.sendTeamTaskMessage.mockResolvedValue({
      message_id: 51,
      conversation_id: "conv-all",
      task_id: "task-all",
      from_actor_id: "user:test",
      to_actor_id: "worker-1",
      route: "to_member",
      payload: {
        type: "chat_message",
        text: "<at>worker-1</at> please inspect the patch",
        mention_actor_ids: ["worker-1"],
      },
      created_at: 1_700_000_051,
    });

    let captured: TeamConversationActions | null = null;
    const options = createOptions();
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.sendTaskMessage({
          text: "<at>worker-1</at> please inspect the patch",
          mentionActorIds: ["worker-1"],
        });
        await Promise.resolve();
      });

      expect(mockedApi.sendTeamTaskMessage).toHaveBeenCalledWith(
        "token-1",
        "team-1",
        "task-all",
        {
          payload: {
            type: "chat_message",
            text: "<at>worker-1</at> please inspect the patch",
            source: "team_workbench",
            mention_actor_ids: ["worker-1"],
          },
        }
      );
      expect(options.setTaskMessageDraft).toHaveBeenCalledWith("");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
      expect(options.refreshEvents).toHaveBeenCalledWith("run-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("reuses the stored shared-thread latest run when the conversation is already loaded", async () => {
    mockedApi.sendTeamTaskMessage.mockResolvedValue({
      message_id: 63,
      conversation_id: "conv-all",
      task_id: "task-all",
      from_actor_id: "user:test",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "chat_message",
        text: "follow up on current thread",
      },
      created_at: 1_700_000_063,
    });

    let captured: TeamConversationActions | null = null;
    const options = createOptions({
      activeRunIdForSelectedTeam: null,
      latestRunForSharedConversation: buildRun("run-shared"),
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.sendTaskMessage({
          text: "follow up on current thread",
          mentionActorIds: [],
        });
        await Promise.resolve();
      });

      expect(mockedApi.ensureTeamSharedThread).not.toHaveBeenCalled();
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-shared");
      expect(options.refreshEvents).toHaveBeenCalledWith("run-shared");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("reuses the existing shared thread instead of creating a duplicate", async () => {
    mockedApi.ensureTeamSharedThread.mockResolvedValue({
      task: buildSharedThreadTask(),
      conversation: {
        id: "conv-all",
        team_id: "team-1",
        task_id: "task-all",
        mode: "group_chat",
        topic: "all",
        created_at: 1,
        updated_at: 1,
      },
      latest_run: buildRun("run-new-shared"),
    });
    mockedApi.listTeamTaskMessages.mockResolvedValue([]);
    mockedApi.sendTeamTaskMessage.mockResolvedValue({
      message_id: 77,
      conversation_id: "conv-all",
      task_id: "task-all",
      from_actor_id: "user:test",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "chat_message",
        text: "hello shared thread",
      },
      created_at: 1_700_000_077,
    });

    let captured: TeamConversationActions | null = null;
    const options = createOptions({
      selectedConversation: null,
      activeRunIdForSelectedTeam: null,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.sendTaskMessage({
          text: "hello shared thread",
          mentionActorIds: [],
        });
      });

      expect(mockedApi.ensureTeamSharedThread).toHaveBeenCalledWith("token-1", "team-1");
      expect(mockedApi.sendTeamTaskMessage).toHaveBeenCalledWith(
        "token-1",
        "team-1",
        "task-all",
        {
          payload: {
            type: "chat_message",
            text: "hello shared thread",
            source: "team_workbench",
          },
        }
      );
      expect(options.setSharedConversationLatestRun).toHaveBeenCalledWith(buildRun("run-new-shared"));
    } finally {
      cleanupHarness(root, container);
    }
  });
});
