// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { buildTeamRuntimeSseUrl } from "../../api";
import {
  TEAM_RUNTIME_REFRESH_INTERVAL_MS,
  useTeamRuntimeEffects,
} from "./use_team_runtime_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useTeamRuntimeEffects>[0];

class MockEventSource {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSED = 2;
  static instances: MockEventSource[] = [];

  readonly url: string;
  readyState = MockEventSource.CONNECTING;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent<string>) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  emitOpen() {
    this.readyState = MockEventSource.OPEN;
    this.onopen?.(new Event("open"));
  }

  emitMessage(data: string) {
    this.onmessage?.(new MessageEvent("message", { data }));
  }

  emitError() {
    this.readyState = MockEventSource.CLOSED;
    this.onerror?.(new Event("error"));
  }

  close() {
    this.readyState = MockEventSource.CLOSED;
  }
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
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
    vi.stubGlobal("EventSource", undefined);
    MockEventSource.instances = [];
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

  it("falls back to polling the selected team runtime when SSE is unavailable", async () => {
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

  it("uses team runtime SSE instead of interval polling when connected", async () => {
    vi.stubGlobal("EventSource", MockEventSource);
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    expect(MockEventSource.instances).toHaveLength(1);
    const source = MockEventSource.instances[0];
    expect(source?.url).toBe(
      buildTeamRuntimeSseUrl(window.location.origin, "team-1", "token-1")
    );

    act(() => {
      source.emitOpen();
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS * 2);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).not.toHaveBeenCalled();

    await act(async () => {
      source.emitMessage(
        JSON.stringify({
          type: "team_runtime",
          payload: { team_id: "team-1", source: "poll_delta" },
        })
      );
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(1);
    expect(params.refreshTeamRuntime).toHaveBeenLastCalledWith("team-1");
  });

  it("reenables runtime polling while SSE reconnects after an error", async () => {
    vi.stubGlobal("EventSource", MockEventSource);
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    const source = MockEventSource.instances[0];
    act(() => {
      source.emitOpen();
      source.emitError();
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

  it("allows the first hidden runtime refresh, then pauses until the page is visible again", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(TEAM_RUNTIME_REFRESH_INTERVAL_MS);
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(1);

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
    });

    expect(params.refreshTeamRuntime).toHaveBeenCalledTimes(2);
    expect(params.refreshTeamRuntime).toHaveBeenLastCalledWith("team-1");
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
