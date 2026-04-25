// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TeamRunRecord, TeamRunSnapshotRecord } from "../../api";
import { useTeamRunLifecycleEffects } from "./use_team_run_lifecycle_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamRunLifecycleEffects>[0];

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

function makeSnapshot(teamId: string): TeamRunSnapshotRecord {
  return {
    run: makeRun("run-1", teamId),
    team: {
      id: teamId,
      name: "Team One",
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
      recent_messages: [],
    },
  };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: "team-1",
    runStatusFilter: "all",
    runs: [makeRun("run-1", "team-1")],
    activeRunIdForSelectedTeam: "run-1",
    snapshot: null,
    eventsAutoRefresh: true,
    tab: "events",
    chatInboxActorId: "",
    refreshAgents: vi.fn().mockResolvedValue(undefined),
    refreshTeams: vi.fn().mockResolvedValue(undefined),
    refreshTeamRuns: vi.fn().mockResolvedValue(undefined),
    refreshRun: vi.fn().mockResolvedValue(makeRun("run-1", "team-1")),
    refreshSteps: vi.fn().mockResolvedValue(undefined),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    refreshSnapshot: vi.fn().mockResolvedValue(makeSnapshot("team-1")),
    loadInbox: vi.fn().mockResolvedValue(undefined),
    parseError: vi.fn(() => "parsed-error"),
    setError: vi.fn(),
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
  useTeamRunLifecycleEffects(params);
  return null;
}

describe("useTeamRunLifecycleEffects", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
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

  it("allows the first hidden active-run refresh, then pauses until the page is visible again", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshRunBase = (params.refreshRun as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshEventsBase = (params.refreshEvents as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshSnapshotBase = (
      params.refreshSnapshot as ReturnType<typeof vi.fn>
    ).mock.calls.length;

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase + 1);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase + 1);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase + 1);

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase + 1);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase + 1);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase + 1);

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase + 2);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase + 2);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase + 2);
  });

  it("does not poll active run context while the member ACP tab is selected", async () => {
    const params = createParams({
      tab: "agent_acp",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshRunBase = (params.refreshRun as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshEventsBase = (params.refreshEvents as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshSnapshotBase = (
      params.refreshSnapshot as ReturnType<typeof vi.fn>
    ).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(8000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase);
  });

  it("does not poll active run context while the mailbox tab is selected", async () => {
    const params = createParams({
      tab: "mailbox",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    const refreshRunBase = (params.refreshRun as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshEventsBase = (params.refreshEvents as ReturnType<typeof vi.fn>).mock.calls.length;
    const refreshSnapshotBase = (
      params.refreshSnapshot as ReturnType<typeof vi.fn>
    ).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(8000);
      await Promise.resolve();
    });

    expect(params.refreshRun).toHaveBeenCalledTimes(refreshRunBase);
    expect(params.refreshEvents).toHaveBeenCalledTimes(refreshEventsBase);
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(refreshSnapshotBase);
  });

  it("hydrates only the active snapshot once for member ACP when snapshot is missing", async () => {
    const params = createParams({
      tab: "agent_acp",
      snapshot: null,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(params.refreshRun).not.toHaveBeenCalled();
    expect(params.refreshEvents).not.toHaveBeenCalled();
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(1);
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
  });

  it("hydrates only the active snapshot once for mailbox when snapshot is missing", async () => {
    const params = createParams({
      tab: "mailbox",
      snapshot: null,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(params.refreshRun).not.toHaveBeenCalled();
    expect(params.refreshEvents).not.toHaveBeenCalled();
    expect(params.refreshSnapshot).toHaveBeenCalledTimes(1);
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
  });

  it("skips member ACP snapshot hydration when the current snapshot already matches the active run", async () => {
    const params = createParams({
      tab: "agent_acp",
      snapshot: makeSnapshot("team-1"),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(params.refreshRun).not.toHaveBeenCalled();
    expect(params.refreshEvents).not.toHaveBeenCalled();
    expect(params.refreshSnapshot).not.toHaveBeenCalled();
  });
});
