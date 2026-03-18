// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTeamConversationEffects } from "./use_team_conversation_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamConversationEffects>[0];

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    selectedTeamId: "team-1",
    selectedConversationId: "task-all",
    tab: "conversation",
    eventsAutoRefresh: false,
    refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
    setTaskMessages: vi.fn(),
    setConversationMailboxMessages: vi.fn(),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamConversationEffects(params);
  return null;
}

describe("useTeamConversationEffects", () => {
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

  it("clears conversation state when there is no shared thread selected", async () => {
    const params = createParams({
      selectedConversationId: null,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.setTaskMessages).toHaveBeenCalledWith([]);
    expect(params.setConversationMailboxMessages).toHaveBeenCalledWith([]);
    expect(params.refreshTaskMessages).not.toHaveBeenCalled();
  });

  it("loads the selected shared thread immediately", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledWith("task-all");
  });

  it("polls the shared thread while the conversation tab is active", async () => {
    vi.useFakeTimers();
    const params = createParams({
      eventsAutoRefresh: true,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const baseCalls = (params.refreshTaskMessages as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls + 1);
    expect(params.refreshTaskMessages).toHaveBeenLastCalledWith("task-all");
  });

  it("does not poll when the workspace is not on the conversation tab", async () => {
    vi.useFakeTimers();
    const params = createParams({
      eventsAutoRefresh: true,
      tab: "tasks",
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const baseCalls = (params.refreshTaskMessages as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls);
  });
});
