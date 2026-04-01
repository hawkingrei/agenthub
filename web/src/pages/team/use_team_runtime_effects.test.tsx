// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  TEAM_RUNTIME_REFRESH_INTERVAL_MS,
  useTeamRuntimeEffects,
} from "./use_team_runtime_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamRuntimeEffects>[0];

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: "team-1",
    enabled: true,
    refreshTeamRuntime: vi.fn().mockResolvedValue(undefined),
    onRefreshError: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamRuntimeEffects(params);
  return null;
}

describe("useTeamRuntimeEffects", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.useFakeTimers();
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

  it("polls the selected team runtime every minute when enabled", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(1);
    expect(params.refreshTeamRuntime).toHaveBeenLastCalledWith("team-1");
  });

  it("stays idle when runtime watching is disabled", async () => {
    const params = createParams({ enabled: false });

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).not.toHaveBeenCalled();
  });

  it("forwards polling errors without breaking the interval", async () => {
    const params = createParams({
      refreshTeamRuntime: vi
        .fn()
        .mockRejectedValueOnce(new Error("network down"))
        .mockResolvedValueOnce(undefined),
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.onRefreshError).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(2);
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

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(1);
    expect(params.refreshTeamRuntime).toHaveBeenLastCalledWith("team-1");

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("online"));
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(3);
  });
});
