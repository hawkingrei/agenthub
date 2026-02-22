// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTeamRunListActions } from "./use_team_run_list_actions";
import type { TeamRunStatusFilter } from "./run_helpers";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamRunListActions>[0];
type HookSnapshot = ReturnType<typeof useTeamRunListActions>;

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: "team-1",
    runStatusFilter: "all",
    runsLoading: false,
    runsHasMore: true,
    runsBeforeCreatedAt: 123,
    setError: vi.fn(),
    parseErrorMessage: vi.fn(() => "parsed-error"),
    setTeamRunBrowserByTeam: vi.fn(),
    refreshTeamRuns: vi.fn().mockResolvedValue([]),
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
  const snapshot = useTeamRunListActions(params);
  onSnapshot(snapshot);
  return null;
}

describe("useTeamRunListActions", () => {
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

  it("updates per-team browser state when run status filter changes", () => {
    const params = createParams();
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    act(() => {
      snapshot?.onRunStatusFilterChange("working");
    });

    const setBrowser = params.setTeamRunBrowserByTeam as ReturnType<typeof vi.fn>;
    expect(setBrowser).toHaveBeenCalledTimes(1);
    const updater = setBrowser.mock.calls[0]?.[0] as (
      prev: Record<string, { statusFilter: TeamRunStatusFilter; hasMore: boolean }>
    ) => Record<string, { statusFilter: TeamRunStatusFilter; hasMore: boolean }>;
    const next = updater({
      "team-1": { statusFilter: "all", hasMore: true },
      "team-2": { statusFilter: "working", hasMore: false },
    });
    expect(next["team-1"]).toMatchObject({
      statusFilter: "working",
      hasMore: false,
      beforeCreatedAt: undefined,
    });
    expect(next["team-2"]).toMatchObject({
      statusFilter: "working",
      hasMore: false,
    });
  });

  it("refreshes runs with replace mode and handles errors", async () => {
    const params = createParams({
      runStatusFilter: "failed",
    });
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={params} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onRefreshRuns();
    });

    expect(params.refreshTeamRuns).toHaveBeenCalledWith("team-1", "replace", {
      statusFilter: "failed",
    });
    expect(params.setError).toHaveBeenCalledWith(null);

    const failingParams = createParams({
      refreshTeamRuns: vi.fn().mockRejectedValue(new Error("network-error")),
      parseErrorMessage: vi.fn(() => "friendly-error"),
    });
    act(() => {
      root.render(<HookHarness params={failingParams} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onRefreshRuns();
    });

    expect(failingParams.setError).toHaveBeenCalledWith("friendly-error");
  });

  it("guards load-more when unavailable and uses append mode when allowed", async () => {
    const blockedParams = createParams({
      runsLoading: true,
      runsHasMore: false,
    });
    const onSnapshot = (next: HookSnapshot) => {
      snapshot = next;
    };

    act(() => {
      root.render(<HookHarness params={blockedParams} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onLoadMoreRuns();
    });
    expect(blockedParams.refreshTeamRuns).not.toHaveBeenCalled();

    const allowedParams = createParams({
      runStatusFilter: "completed",
      runsLoading: false,
      runsHasMore: true,
      runsBeforeCreatedAt: 999,
    });
    act(() => {
      root.render(<HookHarness params={allowedParams} onSnapshot={onSnapshot} />);
    });

    await act(async () => {
      await snapshot?.onLoadMoreRuns();
    });
    expect(allowedParams.refreshTeamRuns).toHaveBeenCalledWith("team-1", "append", {
      statusFilter: "completed",
      beforeCreatedAt: 999,
    });
  });
});
