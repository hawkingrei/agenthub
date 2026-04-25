// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TeamRunRecord } from "../../api";
import { useTeamRunEffects } from "./use_team_run_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamRunEffects>[0];

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

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: null,
    runs: [],
    activeRunId: null,
    runStatusFilter: "all",
    eventsAutoRefresh: false,
    tab: "events",
    chatInboxActorId: "",
    refreshTeams: vi.fn().mockResolvedValue(undefined),
    refreshAgents: vi.fn().mockResolvedValue(undefined),
    refreshTeamRuns: vi.fn().mockResolvedValue(undefined),
    refreshRun: vi.fn().mockResolvedValue(makeRun("run-1", "team-1")),
    refreshSteps: vi.fn().mockResolvedValue(undefined),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    loadInbox: vi.fn().mockResolvedValue(undefined),
    parseErrorMessage: vi.fn(() => "parsed-error"),
    setError: vi.fn(),
    setSelectedTeamId: vi.fn(),
    setActiveRunId: vi.fn(),
    setRuns: vi.fn(),
    setEvents: vi.fn(),
    setSteps: vi.fn(),
    setInbox: vi.fn(),
    setSnapshot: vi.fn(),
    setSelectedMemberId: vi.fn(),
    setMemberEvents: vi.fn(),
    setChatSeenByConversation: vi.fn(),
    setChatStickToBottom: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamRunEffects(params);
  return null;
}

describe("useTeamRunEffects", () => {
  let container: HTMLDivElement;
  let root: Root;

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
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("loads teams and clears state when no team is selected", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.refreshTeams).toHaveBeenCalledTimes(1);
    expect(params.refreshAgents).toHaveBeenCalledTimes(1);
    expect(params.refreshTeamRuns).not.toHaveBeenCalled();
    expect(params.setActiveRunId).toHaveBeenCalledWith(null);
    expect(params.setRuns).toHaveBeenCalledWith([]);
    expect(params.setSelectedMemberId).toHaveBeenCalledWith("");
    expect(params.setChatSeenByConversation).toHaveBeenCalledWith({});
    expect(params.setChatStickToBottom).toHaveBeenCalledWith(true);
  });

  it("loads team runs and picks active run from selected team", async () => {
    const params = createParams({
      selectedTeamId: "team-a",
      runs: [makeRun("run-a", "team-a"), makeRun("run-b", "team-b")],
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setError).toHaveBeenCalledWith(null);
    expect(params.refreshTeamRuns).toHaveBeenCalledWith("team-a", "replace", {
      statusFilter: "all",
    });

    const setActiveRunId = params.setActiveRunId as ReturnType<typeof vi.fn>;
    const updater = setActiveRunId.mock.calls.find(
      (call) => typeof call[0] === "function"
    )?.[0] as ((prev: string | null) => string | null) | undefined;

    expect(updater).toBeDefined();
    expect(updater?.(null)).toBe("run-a");
    expect(updater?.("run-a")).toBe("run-a");
  });

  it("loads active run details and realigns selected team when run belongs elsewhere", async () => {
    const params = createParams({
      selectedTeamId: "team-a",
      activeRunId: "run-x",
      refreshRun: vi.fn().mockResolvedValue(makeRun("run-x", "team-b")),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledWith("run-x");
    expect(params.refreshSteps).toHaveBeenCalledWith("run-x");
    expect(params.refreshEvents).toHaveBeenCalledWith("run-x");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-x");
    expect(params.setSelectedTeamId).toHaveBeenCalledWith("team-b");
  });

  it("polls run/events/snapshot on interval for non-mailbox tab", async () => {
    vi.useFakeTimers();
    const params = createParams({
      selectedTeamId: "team-a",
      activeRunId: "run-a",
      eventsAutoRefresh: true,
      tab: "events",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshRunBase = (params.refreshRun as ReturnType<typeof vi.fn>).mock.calls
      .length;
    const refreshEventsBase = (
      params.refreshEvents as ReturnType<typeof vi.fn>
    ).mock.calls.length;
    const refreshSnapshotBase = (
      params.refreshSnapshot as ReturnType<typeof vi.fn>
    ).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase + 1);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase + 1);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase + 1);
    expect(params.loadInbox).not.toHaveBeenCalled();
  });

  it("polls mailbox snapshot and inbox on interval for mailbox tab", async () => {
    vi.useFakeTimers();
    const params = createParams({
      selectedTeamId: "team-a",
      activeRunId: "run-a",
      eventsAutoRefresh: true,
      tab: "mailbox",
      chatInboxActorId: "  leader-actor  ",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshRunBase = (params.refreshRun as ReturnType<typeof vi.fn>).mock.calls
      .length;
    const refreshEventsBase = (
      params.refreshEvents as ReturnType<typeof vi.fn>
    ).mock.calls.length;
    const refreshSnapshotBase = (
      params.refreshSnapshot as ReturnType<typeof vi.fn>
    ).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase + 1);
    expect(params.loadInbox).toHaveBeenCalledWith("leader-actor");
  });
});
