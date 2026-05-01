// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TeamRunRecord, TeamRunSnapshotRecord } from "../../api";
import { useTeamMailboxLifecycleEffects } from "./use_team_mailbox_lifecycle_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamMailboxLifecycleEffects>[0];

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
    coordinator_member_id: memberIds[0] ?? null,
    members: memberIds.map((memberId) => ({
      member_id: memberId,
      role: memberId === memberIds[0] ? "coordinator" : "worker",
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
    snapshot: makeSnapshot(["coordinator-1", "worker-1"]),
    selectedMemberId: "worker-1",
    mailboxActorIds: [],
    activeRunIdForSelectedTeam: "run-1",
    chatInboxActorId: "user",
    tab: "overview",
    chatStickToBottom: false,
    conversationKey: "conv-1",
    conversationLatestMessageId: 10,
    conversationMessagesLength: 1,
    loadInbox: vi.fn().mockResolvedValue(undefined),
    loadMemberEvents: vi.fn().mockResolvedValue(undefined),
    parseError: vi.fn(() => "parsed-error"),
    setError: vi.fn(),
    setSelectedMemberId: vi.fn(),
    setMemberEvents: vi.fn(),
    setInbox: vi.fn(),
    setInboxActorId: vi.fn(),
    setChatStickToBottom: vi.fn(),
    scrollConversationToBottom: vi.fn(),
    markConversationSeen: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamMailboxLifecycleEffects(params);
  return null;
}

describe("useTeamMailboxLifecycleEffects", () => {
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

  it("does not load member events outside mailbox", async () => {
    const params = createParams({ tab: "agent_acp" });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).not.toHaveBeenCalled();
  });

  it("loads member events in mailbox mode", async () => {
    const params = createParams({ tab: "mailbox" });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledWith("replace");
  });
});
