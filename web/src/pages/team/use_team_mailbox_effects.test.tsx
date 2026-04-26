// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TeamRunRecord, TeamRunSnapshotRecord } from "../../api";
import { useTeamMailboxEffects } from "./use_team_mailbox_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamMailboxEffects>[0];

function makeRun(id: string, teamId: string): TeamRunRecord {
  return {
    id,
    team_id: teamId,
    context_id: `ctx-${id}`,
    status: "working",
    input: {},
    created_at: 1,
    started_at: null,
    ended_at: null,
  };
}

function makeSnapshot(memberIds: string[]): TeamRunSnapshotRecord {
  const run = makeRun("run-1", "team-1");
  return {
    run,
    team: {
      id: run.team_id,
      name: "team-1",
      description: null,
      spec: {},
      created_at: 1,
      updated_at: 1,
    },
    leader_member_id: memberIds[0] ?? null,
    members: memberIds.map((memberId) => ({
      member_id: memberId,
      role: memberId === memberIds[0] ? "leader" : "worker",
      model: null,
      prompt: null,
      skills: [],
      pending_inbox_count: 0,
      status: "idle",
      latest_step: null,
      session_status: null,
    })),
    steps: [],
    latest_events: [],
    mailbox: {
      pending: 0,
      delivered: 0,
      dead_letter: 0,
      recent_messages: [],
    },
  };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    snapshot: null,
    selectedMemberId: "",
    activeRunId: null,
    chatInboxActorId: "",
    tab: "overview",
    chatStickToBottom: false,
    conversationKey: "conv-1",
    conversationLatestMessageId: 10,
    conversationMessageCount: 1,
    loadInbox: vi.fn().mockResolvedValue(undefined),
    loadMemberEvents: vi.fn().mockResolvedValue(undefined),
    markConversationSeen: vi.fn(),
    scrollConversationToBottom: vi.fn(),
    parseErrorMessage: vi.fn(() => "parsed-error"),
    setError: vi.fn(),
    setSelectedMemberId: vi.fn(),
    setMemberEvents: vi.fn(),
    setInbox: vi.fn(),
    setInboxActorId: vi.fn(),
    setChatStickToBottom: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamMailboxEffects(params);
  return null;
}

describe("useTeamMailboxEffects", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      callback(0);
      return 1;
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.restoreAllMocks();
  });

  it("resets member selection state when mailbox snapshot is missing", async () => {
    const params = createParams({ snapshot: null, tab: "mailbox" });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setSelectedMemberId).toHaveBeenCalledWith("");
    expect(params.setMemberEvents).toHaveBeenCalledWith([]);
  });

  it("preserves member selection outside mailbox when snapshot is missing", async () => {
    const params = createParams({
      snapshot: null,
      tab: "agent_acp",
      selectedMemberId: "leader-agent",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setSelectedMemberId).not.toHaveBeenCalled();
    expect(params.setMemberEvents).not.toHaveBeenCalledWith([]);
  });

  it("uses snapshot member fallback when selected member is invalid", async () => {
    const params = createParams({
      snapshot: makeSnapshot(["leader-1", "worker-1"]),
      selectedMemberId: "missing-member",
      tab: "mailbox",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setSelectedMemberId).toHaveBeenCalledWith("leader-1");
  });

  it("loads inbox with trimmed actor id and reports inbox fetch errors", async () => {
    const params = createParams({
      activeRunId: "run-1",
      chatInboxActorId: "  actor-1  ",
      loadInbox: vi.fn().mockRejectedValue(new Error("network-error")),
      parseErrorMessage: vi.fn(() => "friendly-error"),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setInboxActorId).toHaveBeenCalledWith("actor-1");
    expect(params.loadInbox).toHaveBeenCalledWith("actor-1");
    expect(params.setError).toHaveBeenCalledWith("friendly-error");
  });

  it("keeps mailbox conversation at bottom and marks latest message as seen", async () => {
    const params = createParams({
      tab: "mailbox",
      chatStickToBottom: true,
      conversationKey: "member-1|member-2",
      conversationLatestMessageId: 77,
      conversationMessageCount: 8,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setChatStickToBottom).toHaveBeenCalledWith(true);
    expect(params.scrollConversationToBottom).toHaveBeenCalled();
    expect(params.markConversationSeen).toHaveBeenCalledWith("member-1|member-2", 77);
  });

  it("loads member events in replace mode only for mailbox and reports errors", async () => {
    const params = createParams({
      tab: "mailbox",
      loadMemberEvents: vi.fn().mockRejectedValue(new Error("fetch-failed")),
      parseErrorMessage: vi.fn(() => "friendly-event-error"),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledWith("replace");
    expect(params.setError).toHaveBeenCalledWith("friendly-event-error");
  });

  it("does not load member events outside mailbox", async () => {
    const params = createParams({
      tab: "agent_acp",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).not.toHaveBeenCalled();
  });
});
