// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  TEAM_TASK_REFRESH_INTERVAL_MS,
  useTeamTaskEffects,
} from "./use_team_task_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamTaskEffects>[0];

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: "team-1",
    enabled: true,
    refreshTasks: vi.fn().mockResolvedValue(undefined),
    onRefreshError: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamTaskEffects(params);
  return null;
}

describe("useTeamTaskEffects", () => {
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

  it("polls task refresh while enabled", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(1);
    expect(params.refreshTasks).toHaveBeenLastCalledWith("team-1");
  });

  it("does not poll when disabled", async () => {
    const params = createParams({ enabled: false });

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTasks).not.toHaveBeenCalled();
  });

  it("allows the first hidden task refresh, then pauses until the page is visible again", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(1);

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(2);
    expect(params.refreshTasks).toHaveBeenLastCalledWith("team-1");
  });

  it("forwards polling errors without breaking the interval", async () => {
    const params = createParams({
      refreshTasks: vi
        .fn()
        .mockRejectedValueOnce(new Error("network down"))
        .mockResolvedValueOnce(undefined),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.onRefreshError).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(TEAM_TASK_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(2);
  });

  it("refreshes immediately when the page regains focus, visibility, or network", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(1);
    expect(params.refreshTasks).toHaveBeenLastCalledWith("team-1");

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("online"));
      await Promise.resolve();
    });

    expect(params.refreshTasks).toHaveBeenCalledTimes(3);
  });
});
