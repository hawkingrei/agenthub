// @vitest-environment jsdom
import React, { act } from "react";
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
});
