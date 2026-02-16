import { describe, expect, it } from "vitest";
import { AcpPermissionRecord } from "./api";
import {
  decidePermissionJump,
  filterPermissionsForAgent,
  resolveRuntimeViewportSize,
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
  it("resolves viewport size with fallback and clamps to positive pixels", () => {
    expect(
      resolveRuntimeViewportSize({ width: 399.6, height: 701.2 }, 800, 500)
    ).toEqual({
      width: 400,
      height: 701,
    });
    expect(resolveRuntimeViewportSize(undefined, 0, -10)).toEqual({
      width: 1,
      height: 1,
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
});
