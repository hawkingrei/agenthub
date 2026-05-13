// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type AgentEvent } from "../../api";
import { useTeamMemberAcpEffects } from "./use_team_member_acp_effects";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamMemberAcpEffects>[0];

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

function buildAgentEvent(
  eventId: number,
  overrides: Partial<AgentEvent> = {}
): AgentEvent {
  return {
    event_id: eventId,
    agent_id: "agent-1",
    session_id: "session-1",
    seq: String(eventId),
    ts: eventId,
    stream: "acp",
    message: `event-${eventId}`,
    ...overrides,
  };
}

function createStateSetter<T>(initial: T) {
  const state = { current: initial };
  const setter = vi.fn((update: React.SetStateAction<T>) => {
    state.current =
      typeof update === "function"
        ? (update as (prev: T) => T)(state.current)
        : update;
  });
  return { state, setter };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    selectedAgentId: "agent-1",
    selectedSessionId: "session-1",
    tab: "agent_acp",
    eventsAutoRefresh: false,
    loadMemberEvents: vi.fn().mockResolvedValue(undefined),
    setMemberEvents: vi.fn(),
    setMemberEventsHasMore: vi.fn(),
    onLiveActivity: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function HookHarness({ params }: { params: HookParams }) {
  useTeamMemberAcpEffects(params);
  return null;
}

describe("useTeamMemberAcpEffects", () => {
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

  it("clears member events when the selected session disappears", async () => {
    const memberEvents = createStateSetter<AgentEvent[]>([buildAgentEvent(1)]);
    const memberEventsHasMore = createStateSetter(true);
    const params = createParams({
      selectedSessionId: null,
      setMemberEvents: memberEvents.setter,
      setMemberEventsHasMore: memberEventsHasMore.setter,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(memberEvents.state.current).toEqual([]);
    expect(memberEventsHasMore.state.current).toBe(false);
    expect(params.loadMemberEvents).not.toHaveBeenCalled();
  });

  it("loads selected member events immediately for ACP tabs", async () => {
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledWith("replace");
  });

  it("dedupes rapid selection refreshes for the same agent session", async () => {
    vi.useFakeTimers();
    const params = createParams();

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <HookHarness
          params={{
            ...params,
            token: "token-2",
          }}
        />
      );
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(1600);
      await Promise.resolve();
    });

    act(() => {
      root.render(
        <HookHarness
          params={{
            ...params,
            token: "token-3",
          }}
        />
      );
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(2);
  });

  it("preserves a queued poll refresh reason after an in-flight selection refresh", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("EventSource", undefined);
    let resolveLoad: (() => void) | null = null;
    const loadMemberEvents = vi
      .fn<HookParams["loadMemberEvents"]>()
      .mockImplementationOnce(
        () =>
          new Promise<void>((resolve) => {
            resolveLoad = resolve;
          })
      )
      .mockResolvedValue(undefined);

    const params = createParams({
      eventsAutoRefresh: true,
      loadMemberEvents,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    expect(loadMemberEvents).toHaveBeenCalledTimes(1);

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(loadMemberEvents).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveLoad?.();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(loadMemberEvents).toHaveBeenCalledTimes(2);
    expect(loadMemberEvents.mock.calls[1]?.[0]).toBe("replace");
  });

  it("polls member ACP when EventSource is unavailable", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("EventSource", undefined);
    const params = createParams({
      eventsAutoRefresh: true,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls + 1);
  });

  it("does not poll member ACP while SSE is still connecting", async () => {
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

    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls);
  });

  it("stops fallback polling once member ACP SSE is connected", async () => {
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
    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      await Promise.resolve();
    });

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls);
  });

  it("falls back to polling when member ACP SSE stays open but stale", async () => {
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
    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      await Promise.resolve();
    });

    await act(async () => {
      vi.advanceTimersByTime(28_000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls);

    await act(async () => {
      vi.advanceTimersByTime(4_000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls + 1);
  });

  it("falls back to polling when member ACP SSE only receives heartbeats", async () => {
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
    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      await Promise.resolve();
    });

    for (let index = 0; index < 8; index += 1) {
      await act(async () => {
        vi.advanceTimersByTime(4_000);
        source.emitMessage("heartbeat");
        await Promise.resolve();
      });
    }

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls + 1);
  });

  it("appends matching SSE lines for the selected agent session", async () => {
    const memberEvents = createStateSetter<AgentEvent[]>([buildAgentEvent(1)]);
    const params = createParams({
      eventsAutoRefresh: true,
      setMemberEvents: memberEvents.setter,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const source = MockEventSource.instances[0];
    await act(async () => {
      source.emitOpen();
      source.emitMessage(
        JSON.stringify({
          type: "batch",
          payload: [
            buildAgentEvent(11),
            buildAgentEvent(12, { session_id: "session-2" }),
          ],
        })
      );
      await Promise.resolve();
    });

    expect(memberEvents.state.current.map((event) => event.event_id)).toEqual([1, 11]);
  });

  it("hydrates deferred ACP SSE payloads from the event API while the member panel is active", async () => {
    const memberEvents = createStateSetter<AgentEvent[]>([buildAgentEvent(1)]);
    const deferredEvent = buildAgentEvent(11, {
      message: JSON.stringify({
        type: "tool_call_update",
        id: "call-1",
        status: "completed",
        deferred_event_id: 11,
        deferred_fields: ["raw_output"],
      }),
    });
    const fullEvent = buildAgentEvent(11, {
      message: JSON.stringify({
        type: "tool_call_update",
        id: "call-1",
        status: "completed",
        raw_output: { stdout: "full output" },
      }),
    });
    const getAgentEvent = vi.spyOn(api, "getAgentEvent").mockResolvedValueOnce(fullEvent);
    const params = createParams({
      eventsAutoRefresh: true,
      setMemberEvents: memberEvents.setter,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const source = MockEventSource.instances[0];
    await act(async () => {
      source.emitOpen();
      source.emitMessage(
        JSON.stringify({
          type: "acp",
          payload: deferredEvent,
        })
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(getAgentEvent).toHaveBeenCalledWith("token-1", "agent-1", 11);
    expect(memberEvents.state.current.find((event) => event.event_id === 11)?.message).toBe(
      fullEvent.message
    );
  });

  it("ignores deferred ACP hints that do not match the persisted event id", async () => {
    const memberEvents = createStateSetter<AgentEvent[]>([buildAgentEvent(1)]);
    const deferredEvent = buildAgentEvent(11, {
      message: JSON.stringify({
        type: "tool_call_update",
        id: "call-1",
        status: "completed",
        deferred_event_id: 99,
        deferred_fields: ["raw_output"],
      }),
    });
    const getAgentEvent = vi.spyOn(api, "getAgentEvent");
    const params = createParams({
      eventsAutoRefresh: true,
      setMemberEvents: memberEvents.setter,
    });

    act(() => {
      root.render(<HookHarness params={params} />);
    });
    await act(async () => {
      await Promise.resolve();
    });

    const source = MockEventSource.instances[0];
    await act(async () => {
      source.emitOpen();
      source.emitMessage(
        JSON.stringify({
          type: "acp",
          payload: deferredEvent,
        })
      );
      await Promise.resolve();
    });

    expect(getAgentEvent).not.toHaveBeenCalled();
    expect(memberEvents.state.current.find((event) => event.event_id === 11)?.message).toBe(
      deferredEvent.message
    );
  });

  it("syncs related ACP consumers after matching SSE activity", async () => {
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
    await act(async () => {
      source.emitOpen();
      source.emitMessage(
        JSON.stringify({
          type: "batch",
          payload: [buildAgentEvent(11)],
        })
      );
      await Promise.resolve();
    });

    expect(params.onLiveActivity).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(500);
      await Promise.resolve();
    });

    expect(params.onLiveActivity).toHaveBeenCalledTimes(1);
  });

  it("resumes fallback polling after member ACP SSE errors", async () => {
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
    const baseCalls = (params.loadMemberEvents as ReturnType<typeof vi.fn>).mock.calls.length;

    await act(async () => {
      source.emitOpen();
      source.emitError();
      await Promise.resolve();
    });

    await act(async () => {
      vi.advanceTimersByTime(4000);
      await Promise.resolve();
    });

    expect(params.loadMemberEvents).toHaveBeenCalledTimes(baseCalls + 1);
  });
});
