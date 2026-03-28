// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SseConnectionState } from "../../connection_status";
import { useTeamConversationEffects } from "./use_team_conversation_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamConversationEffects>[0];

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
    selectedConversationId: "task-all",
    tab: "conversation",
    eventsAutoRefresh: false,
    refreshTaskMessages: vi.fn().mockResolvedValue(undefined),
    setTaskMessages: vi.fn(),
    setConversationMailboxMessages: vi.fn(),
    onSseStateChange: vi.fn<(nextState: SseConnectionState) => void>(),
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
    MockEventSource.instances = [];
    vi.stubGlobal("EventSource", MockEventSource);
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

  it("refreshes the shared thread when a matching sse message arrives", async () => {
    const params = createParams({
      eventsAutoRefresh: true,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(MockEventSource.instances).toHaveLength(1);
    const source = MockEventSource.instances[0];
    const baseCalls = (params.refreshTaskMessages as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      source.emitMessage(
        JSON.stringify({
          type: "team_conversation",
          payload: { task_id: "task-all", source: "conversation_message" },
        })
      );
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls + 1);
    expect(params.refreshTaskMessages).toHaveBeenLastCalledWith("task-all");
  });

  it("keeps polling while the sse connection is open as a fallback", async () => {
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

    const source = MockEventSource.instances[0];
    const baseCalls = (params.refreshTaskMessages as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls + 1);
  });

  it("reports sse state transitions to the caller", async () => {
    const onSseStateChange = vi.fn<(nextState: SseConnectionState) => void>();
    const params = createParams({
      eventsAutoRefresh: true,
      onSseStateChange,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(onSseStateChange).toHaveBeenCalledWith("connecting");
    const source = MockEventSource.instances[0];

    await act(async () => {
      source.emitOpen();
      await Promise.resolve();
    });

    expect(onSseStateChange).toHaveBeenCalledWith("connected");

    await act(async () => {
      source.emitError();
      await Promise.resolve();
    });

    expect(onSseStateChange).toHaveBeenCalledWith("reconnecting");
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

  it("refreshes the shared thread immediately when the page regains focus, visibility, or network", async () => {
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
      window.dispatchEvent(new Event("focus"));
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls + 1);

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });

    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("online"));
      await Promise.resolve();
    });

    expect(params.refreshTaskMessages).toHaveBeenCalledTimes(baseCalls + 3);
  });

  it("queues a follow-up refresh when selection changes mid-flight", async () => {
    let resolveRefresh: (() => void) | null = null;
    const refreshTaskMessages = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveRefresh = resolve;
        })
    );
    const params = createParams({
      refreshTaskMessages,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(refreshTaskMessages).toHaveBeenCalledTimes(1);
    expect(refreshTaskMessages).toHaveBeenLastCalledWith("task-all");

    const nextParams = createParams({
      refreshTaskMessages,
      selectedConversationId: "task-next",
    });

    act(() => {
      root.render(<HookHarness params={nextParams} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(refreshTaskMessages).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveRefresh?.();
      await Promise.resolve();
    });

    expect(refreshTaskMessages).toHaveBeenCalledTimes(2);
    expect(refreshTaskMessages).toHaveBeenLastCalledWith("task-next");
  });
});
