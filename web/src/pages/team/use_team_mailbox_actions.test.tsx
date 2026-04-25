// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type TeamActorMessageRecord, type TeamRunSnapshotRecord, api } from "../../api";
import type { TeamTab } from "./state";
import { useTeamMailboxActions } from "./use_team_mailbox_actions";

vi.mock("../../api", () => ({
  api: {
    sendTeamRunMessage: vi.fn(),
    ackTeamRunMessage: vi.fn(),
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type TeamMailboxActions = ReturnType<typeof useTeamMailboxActions>;
type TeamMailboxActionsInput = Parameters<typeof useTeamMailboxActions>[0];

type HookHarnessProps = {
  options: TeamMailboxActionsInput;
  onCapture: (actions: TeamMailboxActions) => void;
};

function HookHarness(props: HookHarnessProps) {
  const { options, onCapture } = props;
  const actions = useTeamMailboxActions(options);
  useEffect(() => {
    onCapture(actions);
  }, [actions, onCapture]);
  return null;
}

function createBaseOptions(
  overrides: Partial<TeamMailboxActionsInput> = {}
): TeamMailboxActionsInput {
  const options: TeamMailboxActionsInput = {
    token: "token-1",
    tab: "mailbox",
    activeRunIdForSelectedTeam: "run-1",
    chatFromActorId: "leader-1",
    chatToActorId: "worker-1",
    chatDraft: "hello worker",
    msgFromActorId: "leader-1",
    msgToActorId: "worker-1",
    msgChannel: "default",
    msgTransport: "local",
    msgRoute: '{"route":"inproc"}',
    msgPayload: '{"kind":"task"}',
    msgIdempotencyKey: "idem-1",
    inboxActorId: "worker-1",
    setBusy: vi.fn(),
    setError: vi.fn(),
    setChatDraft: vi.fn(),
    loadInbox: vi.fn(async () => undefined),
    refreshSnapshot: vi.fn(async () => {
      return {
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
        leader_member_id: "leader-1",
        members: [],
        steps: [],
        latest_events: [],
        mailbox: {
          pending: 0,
          delivered: 0,
          dead_letter: 0,
          recent_messages: [],
        },
      } as TeamRunSnapshotRecord;
    }),
    refreshEvents: vi.fn(async () => undefined),
  };
  return { ...options, ...overrides };
}

async function mountHarness(
  options: TeamMailboxActionsInput,
  onCapture: (actions: TeamMailboxActions) => void
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

function buildMessage(overrides: Partial<TeamActorMessageRecord> = {}): TeamActorMessageRecord {
  return {
    message_id: 10,
    run_id: "run-1",
    from_actor_id: "leader-1",
    from_peer_id: "",
    from_actor_kind: "agent",
    to_actor_id: "worker-2",
    to_peer_id: "",
    to_actor_kind: "agent",
    channel: "default",
    transport: "local",
    route: null,
    payload: {},
    status: "pending",
    created_at: 1,
    delivered_at: null,
    ...overrides,
  };
}

describe("useTeamMailboxActions", () => {
  const mockedApi = vi.mocked(api);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("sends chat message and refreshes snapshot + inbox", async () => {
    mockedApi.sendTeamRunMessage.mockResolvedValueOnce(buildMessage());

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions();
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onSendChatMessage();
      });

      expect(mockedApi.sendTeamRunMessage).toHaveBeenCalledWith("token-1", "run-1", {
        from_actor_id: "leader-1",
        to_actor_id: "worker-1",
        channel: "default",
        transport: "local",
        payload: {
          type: "chat_message",
          text: "hello worker",
          source: "team_workbench",
        },
      });
      expect(options.setChatDraft).toHaveBeenCalledWith("");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
      expect(options.loadInbox).toHaveBeenCalledWith("worker-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("sends raw message and branches refresh flow by current tab", async () => {
    mockedApi.sendTeamRunMessage.mockResolvedValueOnce(buildMessage());

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions({
      tab: "events" as TeamTab,
      msgRoute: '{"path":"/mailbox"}',
      msgPayload: '{"kind":"dispatch","value":7}',
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onSendMessage();
      });

      expect(mockedApi.sendTeamRunMessage).toHaveBeenCalledWith("token-1", "run-1", {
        from_actor_id: "leader-1",
        to_actor_id: "worker-1",
        channel: "default",
        transport: "local",
        route: { path: "/mailbox" },
        payload: { kind: "dispatch", value: 7 },
        idempotency_key: "idem-1",
      });
      expect(options.refreshEvents).toHaveBeenCalledWith("run-1");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
      expect(options.loadInbox).not.toHaveBeenCalled();
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("accepts mailbox message with fallback actor id and refreshes non-mailbox views", async () => {
    mockedApi.ackTeamRunMessage.mockResolvedValueOnce(buildMessage({ status: "delivered" }));

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions({
      tab: "debug" as TeamTab,
      inboxActorId: "",
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onAcceptMessage(buildMessage({ message_id: 88, to_actor_id: "worker-x" }));
      });

      expect(mockedApi.ackTeamRunMessage).toHaveBeenCalledWith(
        "token-1",
        "run-1",
        88,
        "worker-x"
      );
      expect(options.loadInbox).toHaveBeenCalledWith();
      expect(options.refreshEvents).toHaveBeenCalledWith("run-1");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("accepts mailbox message with the message recipient even when inbox query actor differs", async () => {
    mockedApi.ackTeamRunMessage.mockResolvedValueOnce(buildMessage({ status: "delivered" }));

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions({
      inboxActorId: "worker-query",
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onAcceptMessage(buildMessage({ message_id: 77, to_actor_id: "worker-real" }));
      });

      expect(mockedApi.ackTeamRunMessage).toHaveBeenCalledWith(
        "token-1",
        "run-1",
        77,
        "worker-real"
      );
      expect(options.loadInbox).toHaveBeenCalledWith("worker-real");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("accepts visible pending messages once and refreshes the selected inbox", async () => {
    mockedApi.ackTeamRunMessage.mockImplementation(
      async (_token, _runId, messageId, actorId) =>
        buildMessage({ message_id: messageId, to_actor_id: actorId, status: "delivered" })
    );

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions();

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onAcceptVisibleMessages([
          buildMessage({ message_id: 51, to_actor_id: "worker-1", status: "pending" }),
          buildMessage({ message_id: 51, to_actor_id: "worker-1", status: "pending" }),
          buildMessage({ message_id: 52, to_actor_id: "worker-1", status: "pending" }),
          buildMessage({ message_id: 99, to_actor_id: "worker-1", status: "delivered" }),
        ]);
      });

      expect(mockedApi.ackTeamRunMessage).toHaveBeenCalledTimes(2);
      expect(mockedApi.ackTeamRunMessage).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "run-1",
        51,
        "worker-1"
      );
      expect(mockedApi.ackTeamRunMessage).toHaveBeenNthCalledWith(
        2,
        "token-1",
        "run-1",
        52,
        "worker-1"
      );
      expect(options.loadInbox).toHaveBeenCalledWith("worker-1");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("accepts visible pending messages with each message recipient instead of the raw inbox query actor", async () => {
    mockedApi.ackTeamRunMessage.mockImplementation(
      async (_token, _runId, messageId, actorId) =>
        buildMessage({ message_id: messageId, to_actor_id: actorId, status: "delivered" })
    );

    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions({
      inboxActorId: "worker-query",
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onAcceptVisibleMessages([
          buildMessage({ message_id: 61, to_actor_id: "worker-a", status: "pending" }),
          buildMessage({ message_id: 62, to_actor_id: "worker-b", status: "pending" }),
        ]);
      });

      expect(mockedApi.ackTeamRunMessage).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "run-1",
        61,
        "worker-a"
      );
      expect(mockedApi.ackTeamRunMessage).toHaveBeenNthCalledWith(
        2,
        "token-1",
        "run-1",
        62,
        "worker-b"
      );
      expect(options.loadInbox).toHaveBeenCalledWith("worker-a");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("returns validation error when refreshing inbox without active run", async () => {
    let captured: TeamMailboxActions | null = null;
    const options = createBaseOptions({
      activeRunIdForSelectedTeam: null,
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      await act(async () => {
        await captured?.onRefreshInbox();
      });

      expect(options.setError).toHaveBeenCalledWith("Select a run in the current team first");
      expect(options.loadInbox).not.toHaveBeenCalled();
    } finally {
      cleanupHarness(root, container);
    }
  });
});
