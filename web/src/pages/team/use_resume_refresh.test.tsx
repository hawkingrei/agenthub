// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useResumeRefresh } from "./use_resume_refresh";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

type HookParams = Parameters<typeof useResumeRefresh>[0];

function HookHarness({ params }: { params: HookParams }) {
  useResumeRefresh(params);
  return null;
}

describe("useResumeRefresh", () => {
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

  it("drops queued follow-up refreshes after the hook is disabled", async () => {
    let resolveRefresh: (() => void) | null = null;
    const refresh = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          resolveRefresh = resolve;
        })
    );

    const params: HookParams = {
      enabled: true,
      intervalMs: null,
      refresh,
      onRefreshError: vi.fn(),
    };

    act(() => {
      root.render(<HookHarness params={params} />);
    });

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });

    expect(refresh).toHaveBeenCalledTimes(1);

    await act(async () => {
      window.dispatchEvent(new Event("online"));
      await Promise.resolve();
    });

    act(() => {
      root.render(
        <HookHarness
          params={{
            ...params,
            enabled: false,
          }}
        />
      );
    });

    await act(async () => {
      resolveRefresh?.();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("pauses interval refreshes while hidden and resumes on visibilitychange", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    act(() => {
      root.render(
        <HookHarness
          params={{
            enabled: true,
            intervalMs: 1000,
            pauseWhenHidden: true,
            refresh,
            onRefreshError: vi.fn(),
          }}
        />
      );
    });

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(refresh).not.toHaveBeenCalled();

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      await Promise.resolve();
    });

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("allows hidden refreshes until the initial refresh succeeds when requested", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });

    act(() => {
      root.render(
        <HookHarness
          params={{
            enabled: true,
            intervalMs: 1000,
            pauseWhenHidden: true,
            pauseWhenHiddenAfterInitialRefresh: true,
            initialRefreshKey: "team-1",
            refresh,
            onRefreshError: vi.fn(),
          }}
        />
      );
    });

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(refresh).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("forwards refresh errors to the latest error handler", async () => {
    const error = new Error("refresh failed");
    const refresh = vi.fn().mockRejectedValue(error);
    const firstHandler = vi.fn();
    const secondHandler = vi.fn();

    act(() => {
      root.render(
        <HookHarness
          params={{
            enabled: true,
            intervalMs: null,
            refresh,
            onRefreshError: firstHandler,
          }}
        />
      );
    });

    act(() => {
      root.render(
        <HookHarness
          params={{
            enabled: true,
            intervalMs: null,
            refresh,
            onRefreshError: secondHandler,
          }}
        />
      );
    });

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(firstHandler).not.toHaveBeenCalled();
    expect(secondHandler).toHaveBeenCalledWith(error);
  });
});
