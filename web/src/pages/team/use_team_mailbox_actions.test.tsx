// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type TeamActorMessageRecord } from "../../api";
import { useTeamMailboxActions } from "./use_team_mailbox_actions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamMailboxActions>[0];
type HookSnapshot = ReturnType<typeof useTeamMailboxActions>;

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    activeRunId: "run-1",
    tab: "mailbox",
    chatActors: {
      fromActorId: "leader",
      toActorId: "worker-1",
      inboxActorId: "worker-1",
    },
    chatDraft: "hello worker",
    msgFromActorId: "leader",
    msgToActorId: "worker-1",
    msgChannel: "default",
    msgTransport: "local",
    msgRoute: "{}",
    msgPayload: "{}",
    msgIdempotencyKey: "idem-1",
    inboxActorId: "worker-1",
    setBusy: vi.fn(),
    setError: vi.fn(),
    parseErrorMessage: vi.fn(() => "friendly-error"),
    setChatDraft: vi.fn(),
    loadInbox: vi.fn().mockResolvedValue(undefined),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function HookHarness({
  params,
  onSnapshot,
}: {
  params: HookParams;
  onSnapshot: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamMailboxActions(params);
  onSnapshot(snapshot);
  return null;
}

describe("useTeamMailboxActions", () => {
  let container: HTMLDivElement;
  let root: Root;
  let snapshot: HookSnapshot | null = null;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    snapshot = null;
    vi.restoreAllMocks();
  });

  it("validates active run before sending chat message", async () => {
    const params = createParams({ activeRunId: null });
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onSendChatMessage();
    });

    expect(params.setError).toHaveBeenCalledWith("Select a run first");
  });

  it("sends chat message and refreshes mailbox conversation state", async () => {
    const params = createParams({
      chatDraft: "  hello team  ",
      chatActors: {
        fromActorId: "leader  ",
        toActorId: "worker-2  ",
        inboxActorId: "worker-2",
      },
    });
    const sendSpy = vi
      .spyOn(api, "sendTeamRunMessage")
      .mockResolvedValue({ status: "ok" } as never);
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onSendChatMessage();
    });

    expect(sendSpy).toHaveBeenCalledWith("token-1", "run-1", {
      from_actor_id: "leader",
      to_actor_id: "worker-2",
      channel: "default",
      transport: "local",
      payload: {
        type: "chat_message",
        text: "hello team",
        source: "team_workbench",
      },
    });
    expect(params.setChatDraft).toHaveBeenCalledWith("");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
    expect(params.loadInbox).toHaveBeenCalledWith("worker-2");
    expect(params.setBusy).toHaveBeenCalledWith("send-chat");
    expect(params.setBusy).toHaveBeenCalledWith(null);
  });

  it("sends structured message and refreshes run events outside mailbox tab", async () => {
    const params = createParams({
      tab: "events",
      msgChannel: "  ",
      msgPayload: '{"task":"do-it"}',
      msgRoute: '{"kind":"local"}',
      msgIdempotencyKey: "  ",
    });
    const sendSpy = vi
      .spyOn(api, "sendTeamRunMessage")
      .mockResolvedValue({ status: "ok" } as never);
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onSendMessage();
    });

    expect(sendSpy).toHaveBeenCalledWith("token-1", "run-1", {
      from_actor_id: "leader",
      to_actor_id: "worker-1",
      channel: undefined,
      transport: "local",
      route: { kind: "local" },
      payload: { task: "do-it" },
      idempotency_key: undefined,
    });
    expect(params.refreshEvents).toHaveBeenCalledWith("run-1");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
  });

  it("acks message with fallback actor id in mailbox tab", async () => {
    const params = createParams({
      inboxActorId: "   ",
      tab: "mailbox",
    });
    const ackSpy = vi
      .spyOn(api, "ackTeamRunMessage")
      .mockResolvedValue({ status: "ok" } as never);
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };
    const message: TeamActorMessageRecord = {
      message_id: 42,
      run_id: "run-1",
      from_actor_id: "leader",
      to_actor_id: "worker-9",
      channel: "default",
      transport: "local",
      payload: { hello: "world" },
      status: "pending",
      created_at: 12345,
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onAckMessage(message);
    });

    expect(ackSpy).toHaveBeenCalledWith("token-1", "run-1", 42, "worker-9");
    expect(params.loadInbox).toHaveBeenCalledWith("worker-9");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
  });
});
