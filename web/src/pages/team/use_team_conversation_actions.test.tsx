// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  type TeamActorMessageRecord,
  type TeamConversationMessageRecord,
  type TeamRunRecord,
  type TeamTaskDetailResponse,
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

function buildTaskThreadTask(
  overrides: Partial<TeamTaskRecord> = {}
): TeamTaskRecord {
  return {
    id: "task-thread",
    team_id: "team-1",
    title: "Task thread",
    status: "in_review",
    created_by_actor_id: "leader",
    assigned_member_id: "worker-1",
    context: {},
    created_at: 2,
    updated_at: 2,
    ...overrides,
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

function buildSharedThreadDetail(
  overrides: Partial<TeamTaskDetailResponse> = {}
): TeamTaskDetailResponse {
  return {
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
    latest_run: buildRun("run-shared"),
    ...overrides,
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
    selectedConversationLatestRun: null,
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

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
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
    mockedApi.getTeamTask.mockResolvedValue(
      buildSharedThreadDetail({
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
      })
    );
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

  it("reuses the selected conversation latest run when refreshing messages", async () => {
    mockedApi.listTeamTaskMessages.mockResolvedValueOnce([
      buildTaskMessage(28, "tail-1"),
      buildTaskMessage(31, "tail-2"),
    ]);
    mockedApi.getTeamRunSnapshot.mockResolvedValueOnce({
      run: buildRun("run-selected"),
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
        recent_messages: [buildMailboxMessage(99, "latest mailbox")],
      },
    } as Awaited<ReturnType<typeof api.getTeamRunSnapshot>>);

    let captured: TeamConversationActions | null = null;
    const options = createOptions({
      selectedConversationLatestRun: buildRun("run-selected"),
    });
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

      expect(mockedApi.getTeamTask).not.toHaveBeenCalled();
      expect(mockedApi.getTeamRunSnapshot).toHaveBeenCalledWith(
        "token-1",
        "run-selected",
        { event_limit: 1, message_limit: 20 }
      );
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("keeps shared-thread message state bounded to recent-20 across repeated refreshes", async () => {
    const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
    const mailboxMessages = createStateSetter<TeamActorMessageRecord[]>([]);
    mockedApi.listTeamTaskMessages
      .mockResolvedValueOnce(
        Array.from({ length: 20 }, (_, index) => buildTaskMessage(index + 1, `wave-1-${index + 1}`))
      )
      .mockResolvedValueOnce(
        Array.from({ length: 20 }, (_, index) => buildTaskMessage(index + 11, `wave-2-${index + 11}`))
      );
    mockedApi.getTeamTask.mockResolvedValue(
      buildSharedThreadDetail({
        latest_run: buildRun("run-1"),
      })
    );
    mockedApi.getTeamRunSnapshot.mockResolvedValue({
      run: buildRun("run-1"),
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
    const options = createOptions({
      setTaskMessages: taskMessages.setter,
      setConversationMailboxMessages: mailboxMessages.setter,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.refreshTaskMessages();
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(taskMessages.state.current).toHaveLength(20);
      expect(taskMessages.state.current[0]?.message_id).toBe(1);
      expect(taskMessages.state.current.at(-1)?.message_id).toBe(20);

      await act(async () => {
        await captured?.refreshTaskMessages();
        await Promise.resolve();
        await Promise.resolve();
      });
      expect(taskMessages.state.current).toHaveLength(20);
      expect(taskMessages.state.current[0]?.message_id).toBe(11);
      expect(taskMessages.state.current.at(-1)?.message_id).toBe(30);
      expect(mailboxMessages.state.current).toHaveLength(1);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("clears stale messages immediately when switching conversation scope", async () => {
    let captured: TeamConversationActions | null = null;
    const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
    const mailboxMessages = createStateSetter<TeamActorMessageRecord[]>([]);
    const options = createOptions({
      setTaskMessages: taskMessages.setter,
      setConversationMailboxMessages: mailboxMessages.setter,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      taskMessages.state.current = [buildTaskMessage(11, "shared thread message")];
      mailboxMessages.state.current = [buildMailboxMessage(12, "shared mailbox message")];
      taskMessages.setter.mockClear();
      mailboxMessages.setter.mockClear();

      const nextOptions = createOptions({
        ...options,
        selectedConversation: buildTaskThreadTask(),
        setTaskMessages: taskMessages.setter,
        setConversationMailboxMessages: mailboxMessages.setter,
      });

      await act(async () => {
        root.render(<HookHarness options={nextOptions} onCapture={(actions) => {
          captured = actions;
        }} />);
        await Promise.resolve();
      });

      expect(captured).not.toBeNull();
      expect(taskMessages.state.current).toEqual([]);
      expect(mailboxMessages.state.current).toEqual([]);
      expect(taskMessages.setter).toHaveBeenCalledWith([]);
      expect(mailboxMessages.setter).toHaveBeenCalledWith([]);
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
          idempotency_key: expect.any(String),
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
      selectedConversationLatestRun: buildRun("run-shared"),
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
    mockedApi.ensureTeamSharedThread.mockResolvedValue(
      buildSharedThreadDetail({
        latest_run: buildRun("run-new-shared"),
      })
    );
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
          idempotency_key: expect.any(String),
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

  it("optimistically echoes a pending channel send and ignores duplicate submits while in flight", async () => {
    const deferred = createDeferred<TeamConversationMessageRecord>();
    mockedApi.sendTeamTaskMessage.mockReturnValueOnce(deferred.promise);

    let captured: TeamConversationActions | null = null;
    const draft = createStateSetter("hello team");
    const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
    const options = createOptions({
      setTaskMessages: taskMessages.setter,
      setTaskMessageDraft: draft.setter,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        void captured?.sendTaskMessage({
          text: "hello team",
          mentionActorIds: [],
        });
        void captured?.sendTaskMessage({
          text: "hello team",
          mentionActorIds: [],
        });
        await Promise.resolve();
      });

      expect(mockedApi.sendTeamTaskMessage).toHaveBeenCalledTimes(1);
      expect(draft.state.current).toBe("");
      expect(taskMessages.state.current).toHaveLength(1);
      expect(taskMessages.state.current[0]?.from_actor_id).toBe("user");
      expect(taskMessages.state.current[0]?.conversation_id).toBe("");
      expect(taskMessages.state.current[0]?.payload).toEqual({
        type: "chat_message",
        text: "hello team",
        source: "team_workbench",
      });

      deferred.resolve(buildTaskMessage(91, "hello team"));
      await act(async () => {
        await deferred.promise;
        await Promise.resolve();
      });

      expect(taskMessages.state.current).toEqual([buildTaskMessage(91, "hello team")]);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("restores the draft and removes the optimistic echo when sending fails", async () => {
    const deferred = createDeferred<TeamConversationMessageRecord>();
    mockedApi.sendTeamTaskMessage.mockReturnValueOnce(deferred.promise);

    let captured: TeamConversationActions | null = null;
    const draft = createStateSetter("need retry");
    const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
    const options = createOptions({
      setTaskMessages: taskMessages.setter,
      setTaskMessageDraft: draft.setter,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        void captured?.sendTaskMessage({
          text: "need retry",
          mentionActorIds: [],
        });
        await Promise.resolve();
      });

      draft.setter("newer text");

      deferred.reject(new Error("network broke"));
      await act(async () => {
        try {
          await deferred.promise;
        } catch {
          // expected rejection
        }
        await Promise.resolve();
      });

      expect(taskMessages.state.current).toEqual([]);
      expect(draft.state.current).toBe("need retry\nnewer text");
      expect(options.setError).toHaveBeenCalledWith("network broke");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("uses the ensured shared-thread conversation id for the optimistic echo", async () => {
    const ensureDeferred = createDeferred<TeamTaskDetailResponse>();
    const sendDeferred = createDeferred<TeamConversationMessageRecord>();
    mockedApi.ensureTeamSharedThread.mockReturnValueOnce(ensureDeferred.promise);
    mockedApi.sendTeamTaskMessage.mockReturnValueOnce(sendDeferred.promise);

    let captured: TeamConversationActions | null = null;
    const draft = createStateSetter("hello shared thread");
    const taskMessages = createStateSetter<TeamConversationMessageRecord[]>([]);
    const options = createOptions({
      selectedConversation: null,
      activeRunIdForSelectedTeam: null,
      setTaskMessages: taskMessages.setter,
      setTaskMessageDraft: draft.setter,
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        void captured?.sendTaskMessage({
          text: "hello shared thread",
          mentionActorIds: [],
        });
        await Promise.resolve();
      });

      expect(draft.state.current).toBe("");
      draft.setter("follow-up draft");

      ensureDeferred.resolve(buildSharedThreadDetail());
      await act(async () => {
        await ensureDeferred.promise;
        await Promise.resolve();
      });

      expect(taskMessages.state.current).toHaveLength(1);
      expect(taskMessages.state.current[0]?.conversation_id).toBe("conv-all");

      sendDeferred.resolve(buildTaskMessage(101, "hello shared thread"));
      await act(async () => {
        await sendDeferred.promise;
        await Promise.resolve();
      });

      expect(draft.state.current).toBe("follow-up draft");
    } finally {
      cleanupHarness(root, container);
    }
  });
});
