import { describe, expect, it, vi } from "vitest";
import { AcpPermissionRecord, AgentEvent } from "./api";
import {
  buildGlobalPermissionPollAgentIds,
  buildPendingPermissionCountMap,
  chunkPermissionPollAgentIds,
  decidePermissionJump,
  filterPermissionsForAgent,
  mergePendingPermissionCountMap,
  parsePermissionPollAgentIds,
  resolveGlobalPermissionPollIntervalMs,
  resolveOutputHistoryKey,
  resolveSessionScopedEvents,
  resolveRuntimeViewportAxis,
  resolveRuntimeViewportSize,
  schedulePermissionPollLoop,
  setupLayoutAnchorVarSync,
  setupRuntimeViewportVarSync,
  shouldSyncRuntimeViewportSize,
  toNonNegativeRoundedPx,
} from "./app";

const buildPermission = (
  id: string,
  agentId: string,
  status = "pending"
): AcpPermissionRecord => ({
  id,
  agent_id: agentId,
  session_id: `${agentId}-session`,
  options: [],
  status,
  created_at: 1,
});

const buildEvent = (
  eventId: number,
  sessionId: string,
  stream: AgentEvent["stream"] = "stdout"
): AgentEvent => ({
  event_id: eventId,
  agent_id: "agent-1",
  session_id: sessionId,
  seq: String(eventId),
  ts: eventId,
  stream,
  message: `${stream}-${eventId}`,
});

class MockEventTarget {
  private listeners = new Map<string, Set<() => void>>();

  addEventListener(type: string, listener: () => void) {
    let set = this.listeners.get(type);
    if (!set) {
      set = new Set();
      this.listeners.set(type, set);
    }
    set.add(listener);
  }

  removeEventListener(type: string, listener: () => void) {
    this.listeners.get(type)?.delete(listener);
  }

  emit(type: string) {
    const set = this.listeners.get(type);
    if (!set) return;
    for (const listener of set) {
      listener();
    }
  }

  listenerCount(type: string): number {
    return this.listeners.get(type)?.size ?? 0;
  }
}

function createStyleTarget() {
  const values = new Map<string, string>();
  return {
    values,
    target: {
      setProperty: (name: string, value: string) => {
        values.set(name, value);
      },
    },
  };
}

describe("filterPermissionsForAgent", () => {
  it("returns empty when active agent is null", () => {
    const input = [buildPermission("p1", "agent-a")];
    expect(filterPermissionsForAgent(input, null)).toEqual([]);
  });

  it("keeps only permission records that belong to active agent", () => {
    const input = [
      buildPermission("p1", "agent-a"),
      buildPermission("p2", "agent-b", "responded"),
      buildPermission("p3", "agent-a", "timeout"),
    ];
    expect(filterPermissionsForAgent(input, "agent-a").map((item) => item.id)).toEqual([
      "p1",
      "p3",
    ]);
  });
});

describe("app helper decisions", () => {
  it("parses permission poll agent ids from stable key", () => {
    expect(parsePermissionPollAgentIds("")).toEqual([]);
    expect(parsePermissionPollAgentIds("agent-a,agent-b")).toEqual([
      "agent-a",
      "agent-b",
    ]);
    expect(parsePermissionPollAgentIds("agent-a,,agent-b,")).toEqual([
      "agent-a",
      "agent-b",
    ]);
  });

  it("builds global permission poll targets and skips active agent", () => {
    const allAgentIds = ["agent-a", "agent-b", "agent-c"];
    expect(buildGlobalPermissionPollAgentIds(allAgentIds, null)).toEqual(
      allAgentIds
    );
    expect(buildGlobalPermissionPollAgentIds(allAgentIds, "agent-b")).toEqual([
      "agent-a",
      "agent-c",
    ]);
  });

  it("splits permission poll ids into bounded concurrent chunks", () => {
    expect(
      chunkPermissionPollAgentIds(
        ["agent-a", "agent-b", "agent-c", "agent-d", "agent-e"],
        2
      )
    ).toEqual([
      ["agent-a", "agent-b"],
      ["agent-c", "agent-d"],
      ["agent-e"],
    ]);
    expect(chunkPermissionPollAgentIds(["agent-a", "agent-b"], 0)).toEqual([
      ["agent-a"],
      ["agent-b"],
    ]);
  });

  it("uses slower global permission polling interval when agents panel is collapsed", () => {
    expect(resolveGlobalPermissionPollIntervalMs(false)).toBe(5000);
    expect(resolveGlobalPermissionPollIntervalMs(true)).toBe(10000);
  });

  it("builds pending permission count map using only positive counts", () => {
    const map = buildPendingPermissionCountMap([
      ["agent-a", 2],
      ["agent-b", 0],
      ["agent-c", -1],
      ["agent-d", 1],
    ]);
    expect(map).toEqual({
      "agent-a": 2,
      "agent-d": 1,
    });
  });

  it("merges pending permission counts while preserving failed agents", () => {
    const prev = {
      "agent-a": 3,
      "agent-b": 2,
      "agent-z": 9,
    };
    const next = mergePendingPermissionCountMap(
      prev,
      ["agent-a", "agent-b", "agent-c"],
      [
        ["agent-a", 0],
        ["agent-b", null],
        ["agent-c", 1],
      ]
    );
    expect(next).toEqual({
      "agent-b": 2,
      "agent-c": 1,
    });
  });

  it("preserves non-polled agent counts across active-agent switches", () => {
    const initial = {
      "agent-a": 3,
      "agent-b": 2,
    };

    const afterFirstGlobalPoll = mergePendingPermissionCountMap(
      initial,
      ["agent-a", "agent-b", "agent-c"],
      [
        ["agent-a", 0],
        ["agent-c", 1],
      ]
    );
    expect(afterFirstGlobalPoll).toEqual({
      "agent-b": 2,
      "agent-c": 1,
    });

    const afterSwitchGlobalPoll = mergePendingPermissionCountMap(
      afterFirstGlobalPoll,
      ["agent-a", "agent-b", "agent-c"],
      [
        ["agent-a", 4],
        ["agent-b", 1],
      ]
    );
    expect(afterSwitchGlobalPoll).toEqual({
      "agent-a": 4,
      "agent-b": 1,
      "agent-c": 1,
    });
  });

  it("keeps permission lists scoped when active agent changes", () => {
    const permissions = [
      buildPermission("p-a-1", "agent-a"),
      buildPermission("p-b-1", "agent-b"),
      buildPermission("p-a-2", "agent-a", "responded"),
      buildPermission("p-c-1", "agent-c"),
    ];

    expect(
      filterPermissionsForAgent(permissions, "agent-a").map((item) => item.id)
    ).toEqual(["p-a-1", "p-a-2"]);
    expect(
      filterPermissionsForAgent(permissions, "agent-b").map((item) => item.id)
    ).toEqual(["p-b-1"]);
  });

  it("drops stale counts for agents removed from the current list", () => {
    const prev = {
      "agent-a": 2,
      "agent-b": 1,
    };
    const next = mergePendingPermissionCountMap(
      prev,
      ["agent-a"],
      [["agent-a", 2]]
    );
    expect(next).toEqual({
      "agent-a": 2,
    });
  });

  it("builds output history key with latest session marker", () => {
    expect(resolveOutputHistoryKey("agent-a", "session-1", {})).toBe(
      "agent-a:session-1"
    );
    expect(resolveOutputHistoryKey("agent-a", null, { "agent-a": "session-latest" })).toBe(
      "agent-a:latest:session-latest"
    );
    expect(resolveOutputHistoryKey("agent-a", null, {})).toBe(
      "agent-a:latest:latest"
    );
  });

  it("resolves scoped events for explicit session id", () => {
    const events = [
      buildEvent(1, "session-a"),
      buildEvent(2, "session-b"),
      buildEvent(3, "session-a"),
    ];
    const resolved = resolveSessionScopedEvents(events, "session-a");
    expect(resolved.latestSessionId).toBeNull();
    expect(resolved.resolvedSessionId).toBe("session-a");
    expect(resolved.scopedEvents.map((item) => item.event_id)).toEqual([1, 3]);
  });

  it("resolves latest session and scopes events when session id is missing", () => {
    const events = [
      buildEvent(1, "session-a"),
      buildEvent(2, "session-b"),
      buildEvent(3, "session-b"),
      buildEvent(4, "session-c"),
    ];
    const resolved = resolveSessionScopedEvents(events, null);
    expect(resolved.latestSessionId).toBe("session-c");
    expect(resolved.resolvedSessionId).toBe("session-c");
    expect(resolved.scopedEvents.map((item) => item.event_id)).toEqual([4]);
  });

  it("resolves latest session from event ordering even when list order is stale", () => {
    const events = [
      buildEvent(10, "session-new"),
      buildEvent(3, "session-old"),
      buildEvent(8, "session-mid"),
    ];
    const resolved = resolveSessionScopedEvents(events, null);
    expect(resolved.latestSessionId).toBe("session-new");
    expect(resolved.resolvedSessionId).toBe("session-new");
    expect(resolved.scopedEvents.map((item) => item.event_id)).toEqual([10]);
  });

  it("keeps all events when no session can be resolved", () => {
    const events = [
      { ...buildEvent(1, ""), session_id: "" },
      { ...buildEvent(2, ""), session_id: "" },
    ];
    const resolved = resolveSessionScopedEvents(events, null);
    expect(resolved.latestSessionId).toBeNull();
    expect(resolved.resolvedSessionId).toBeNull();
    expect(resolved.scopedEvents.map((item) => item.event_id)).toEqual([1, 2]);
  });

  it("resolves viewport size with fallback and clamps to positive pixels", () => {
    expect(
      resolveRuntimeViewportSize({ width: 399.6, height: 701.2 }, 800, 500)
    ).toEqual({
      width: 400,
      height: 701,
    });
    expect(resolveRuntimeViewportSize({ width: 1, height: 0 }, 812, 390)).toEqual({
      width: 390,
      height: 812,
    });
    expect(
      resolveRuntimeViewportSize(
        { width: Number.NaN, height: Number.NEGATIVE_INFINITY },
        844,
        412
      )
    ).toEqual({
      width: 412,
      height: 844,
    });
    expect(resolveRuntimeViewportSize(undefined, 0, -10)).toEqual({
      width: 1,
      height: 1,
    });
  });

  it("ignores implausibly tiny visual viewport values and keeps fallback axes", () => {
    expect(resolveRuntimeViewportAxis(3, 390)).toBe(390);
    expect(resolveRuntimeViewportAxis(320, 844)).toBe(320);
    expect(
      resolveRuntimeViewportSize({ width: 3, height: 9 }, 844, 390)
    ).toEqual({
      width: 390,
      height: 844,
    });
  });

  it("detects when runtime viewport css vars should sync", () => {
    const next = { width: 500, height: 700 };
    expect(shouldSyncRuntimeViewportSize(null, next)).toBe(true);
    expect(shouldSyncRuntimeViewportSize({ width: 500, height: 700 }, next)).toBe(
      false
    );
    expect(shouldSyncRuntimeViewportSize({ width: 501, height: 700 }, next)).toBe(
      true
    );
  });

  it("normalizes px values to non-negative rounded ints", () => {
    expect(toNonNegativeRoundedPx(15.6)).toBe(16);
    expect(toNonNegativeRoundedPx(-4.2)).toBe(0);
    expect(toNonNegativeRoundedPx(null)).toBeNull();
    expect(toNonNegativeRoundedPx(Number.NaN)).toBeNull();
  });

  it("decides permission jump phases", () => {
    expect(decidePermissionJump(null, "conversation", null)).toBe("idle");
    expect(
      decidePermissionJump(
        { toolCallId: "call-1", sessionId: "s1", attempts: 0 },
        "debug",
        "s1"
      )
    ).toBe("wait");
    expect(
      decidePermissionJump(
        { toolCallId: "call-1", sessionId: "s1", attempts: 0 },
        "conversation",
        "s2"
      )
    ).toBe("wait");
    expect(
      decidePermissionJump(
        { toolCallId: "call-1", sessionId: null, attempts: 24 },
        "conversation",
        "s1"
      )
    ).toBe("clear");
    expect(
      decidePermissionJump(
        { toolCallId: "call-1", sessionId: "s1", attempts: 3 },
        "conversation",
        "s1"
      )
    ).toBe("attempt");
  });

  it("syncs viewport css vars and unregisters listeners", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const style = createStyleTarget();

    const cleanup = setupRuntimeViewportVarSync(
      runtimeWindow as unknown as Parameters<typeof setupRuntimeViewportVarSync>[0],
      style.target
    );
    expect(style.values.get("--agenthub-vh")).toBe("700px");
    expect(style.values.get("--agenthub-vw")).toBe("390px");

    runtimeWindow.visualViewport.height = 666;
    runtimeWindow.visualViewport.width = 360;
    runtimeWindow.visualViewport.emit("resize");
    expect(style.values.get("--agenthub-vh")).toBe("666px");
    expect(style.values.get("--agenthub-vw")).toBe("360px");

    cleanup();
    expect(runtimeWindow.listenerCount("resize")).toBe(0);
    expect(runtimeWindow.listenerCount("orientationchange")).toBe(0);
    expect(runtimeWindow.visualViewport.listenerCount("resize")).toBe(0);
    expect(runtimeWindow.visualViewport.listenerCount("scroll")).toBe(0);
  });

  it("coalesces viewport sync with requestAnimationFrame and cancels pending frame on cleanup", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
      requestAnimationFrame: (cb: (ts: number) => void) => number;
      cancelAnimationFrame: (id: number) => void;
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const style = createStyleTarget();
    const rafCallbacks: Array<(ts: number) => void> = [];
    const cancelSpy = vi.fn();
    runtimeWindow.requestAnimationFrame = (cb) => {
      rafCallbacks.push(cb);
      return rafCallbacks.length;
    };
    runtimeWindow.cancelAnimationFrame = cancelSpy;

    const cleanup = setupRuntimeViewportVarSync(
      runtimeWindow as unknown as Parameters<typeof setupRuntimeViewportVarSync>[0],
      style.target
    );

    runtimeWindow.visualViewport.height = 680;
    runtimeWindow.visualViewport.width = 370;
    runtimeWindow.visualViewport.emit("resize");
    runtimeWindow.visualViewport.emit("scroll");
    expect(rafCallbacks.length).toBe(1);
    rafCallbacks[0](0);
    expect(style.values.get("--agenthub-vh")).toBe("680px");
    expect(style.values.get("--agenthub-vw")).toBe("370px");

    runtimeWindow.visualViewport.height = 650;
    runtimeWindow.visualViewport.emit("resize");
    expect(rafCallbacks.length).toBe(2);
    cleanup();
    expect(cancelSpy).toHaveBeenCalledWith(2);
  });

  it("skips viewport css writes when viewport size does not change", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const values = new Map<string, string>();
    const setProperty = vi.fn((name: string, value: string) => {
      values.set(name, value);
    });

    const cleanup = setupRuntimeViewportVarSync(
      runtimeWindow as unknown as Parameters<typeof setupRuntimeViewportVarSync>[0],
      { setProperty }
    );
    expect(values.get("--agenthub-vh")).toBe("700px");
    expect(values.get("--agenthub-vw")).toBe("390px");
    setProperty.mockClear();

    runtimeWindow.visualViewport.emit("resize");
    expect(setProperty).not.toHaveBeenCalled();
    cleanup();
  });

  it("syncs layout anchor vars and disconnects observer on cleanup", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
      requestAnimationFrame: (cb: (ts: number) => void) => number;
      cancelAnimationFrame: (id: number) => void;
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const rafCallbacks: Array<(ts: number) => void> = [];
    runtimeWindow.requestAnimationFrame = (cb) => {
      rafCallbacks.push(cb);
      return rafCallbacks.length;
    };
    runtimeWindow.cancelAnimationFrame = () => {};
    const style = createStyleTarget();
    const headerRect = { height: 56, top: 0 };
    const workspaceRect = { height: 0, top: 64 };
    const appRootRect = { height: 0, top: 0 };
    const observedTargets: object[] = [];
    let disconnected = false;
    class MockResizeObserver {
      private callback: () => void;
      constructor(callback: () => void) {
        this.callback = callback;
      }
      observe(target: object) {
        observedTargets.push(target);
      }
      disconnect() {
        disconnected = true;
      }
      trigger() {
        this.callback();
      }
    }

    const cleanup = setupLayoutAnchorVarSync(
      runtimeWindow as unknown as Parameters<typeof setupLayoutAnchorVarSync>[0],
      style.target,
      {
        appRoot: {
          getBoundingClientRect: () => appRootRect,
        },
        appHeader: {
          getBoundingClientRect: () => headerRect,
        },
        workspace: {
          getBoundingClientRect: () => workspaceRect,
        },
      },
      MockResizeObserver as unknown as Parameters<typeof setupLayoutAnchorVarSync>[3]
    );
    expect(style.values.get("--agenthub-header-height")).toBe("56px");
    expect(style.values.get("--agenthub-workspace-top")).toBe("64px");
    expect(observedTargets.length).toBe(3);

    headerRect.height = 72;
    workspaceRect.top = 88;
    runtimeWindow.emit("resize");
    expect(rafCallbacks.length).toBe(1);
    rafCallbacks[0](0);
    expect(style.values.get("--agenthub-header-height")).toBe("72px");
    expect(style.values.get("--agenthub-workspace-top")).toBe("88px");

    cleanup();
    expect(disconnected).toBe(true);
  });

  it("handles layout sync without raf and without resize observer", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const style = createStyleTarget();
    const cleanup = setupLayoutAnchorVarSync(
      runtimeWindow as unknown as Parameters<typeof setupLayoutAnchorVarSync>[0],
      style.target,
      {
        appRoot: null,
        appHeader: null,
        workspace: null,
      }
    );

    runtimeWindow.emit("resize");
    expect(style.values.get("--agenthub-header-height")).toBeUndefined();
    expect(style.values.get("--agenthub-workspace-top")).toBeUndefined();

    cleanup();
    expect(runtimeWindow.listenerCount("resize")).toBe(0);
    expect(runtimeWindow.listenerCount("orientationchange")).toBe(0);
    expect(runtimeWindow.visualViewport.listenerCount("resize")).toBe(0);
    expect(runtimeWindow.visualViewport.listenerCount("scroll")).toBe(0);
  });

  it("coalesces layout sync by cancelling previous frame and pending cleanup frame", () => {
    const runtimeWindow = new MockEventTarget() as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: MockEventTarget & { height: number; width: number };
      requestAnimationFrame: (cb: (ts: number) => void) => number;
      cancelAnimationFrame: (id: number) => void;
    };
    runtimeWindow.innerHeight = 700;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = Object.assign(new MockEventTarget(), {
      height: 700,
      width: 390,
    });
    const rafCallbacks: Array<(ts: number) => void> = [];
    const cancelSpy = vi.fn();
    runtimeWindow.requestAnimationFrame = (cb) => {
      rafCallbacks.push(cb);
      return rafCallbacks.length;
    };
    runtimeWindow.cancelAnimationFrame = cancelSpy;

    const cleanup = setupLayoutAnchorVarSync(
      runtimeWindow as unknown as Parameters<typeof setupLayoutAnchorVarSync>[0],
      createStyleTarget().target,
      {
        appRoot: null,
        appHeader: {
          getBoundingClientRect: () => ({ height: 56, top: 0 }),
        },
        workspace: {
          getBoundingClientRect: () => ({ height: 0, top: 64 }),
        },
      }
    );

    runtimeWindow.emit("resize");
    runtimeWindow.emit("orientationchange");
    expect(cancelSpy).toHaveBeenCalledWith(1);
    expect(rafCallbacks.length).toBe(2);

    cleanup();
    expect(cancelSpy).toHaveBeenCalledWith(2);
  });

  it("does not schedule permission polling when already cancelled", () => {
    const pollState = { timer: null as number | null };
    const scheduleSpy = vi.fn(() => 1);
    const clearSpy = vi.fn();

    schedulePermissionPollLoop(
      0,
      pollState,
      async () => 0,
      () => true,
      scheduleSpy,
      clearSpy
    );

    expect(scheduleSpy).not.toHaveBeenCalled();
    expect(clearSpy).not.toHaveBeenCalled();
    expect(pollState.timer).toBeNull();
  });

  it("stops permission poll loop when cancelled during an in-flight request", async () => {
    const pollState = { timer: null as number | null };
    const scheduledCallbacks: Array<() => void> = [];
    const clearSpy = vi.fn();
    let cancelled = false;
    let resolvePollOnce: ((count: number) => void) | null = null;
    const pollOnce = vi.fn(
      () =>
        new Promise<number>((resolve) => {
          resolvePollOnce = resolve;
        })
    );

    schedulePermissionPollLoop(
      0,
      pollState,
      pollOnce,
      () => cancelled,
      (callback) => {
        scheduledCallbacks.push(callback);
        return scheduledCallbacks.length;
      },
      clearSpy
    );

    expect(scheduledCallbacks).toHaveLength(1);
    const running = scheduledCallbacks[0]();
    expect(pollOnce).toHaveBeenCalledTimes(1);

    cancelled = true;
    resolvePollOnce?.(1);
    await running;

    expect(scheduledCallbacks).toHaveLength(1);
    expect(clearSpy).not.toHaveBeenCalled();
    expect(pollState.timer).toBeNull();
  });
});
