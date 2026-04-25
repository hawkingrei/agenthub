import { describe, expect, it, vi } from "vitest";
import {
  resolveRuntimeKeyboardInset,
  resolveRuntimeViewportSize,
  setupRuntimeViewportVarSync,
} from "./app_viewport";

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
    for (const listener of this.listeners.get(type) ?? []) {
      listener();
    }
  }
}

function createStyleTarget() {
  const writes: Array<[string, string]> = [];
  return {
    writes,
    target: {
      setProperty(name: string, value: string) {
        writes.push([name, value]);
      },
    },
  };
}

describe("resolveRuntimeViewportSize", () => {
  it("includes visual viewport offsetTop in the effective viewport height", () => {
    expect(
      resolveRuntimeViewportSize(
        { width: 390, height: 700, offsetTop: 44 },
        844,
        390
      )
    ).toEqual({
      height: 744,
      width: 390,
    });
  });
});

describe("resolveRuntimeKeyboardInset", () => {
  it("subtracts visual viewport height and offsetTop from innerHeight", () => {
    expect(
      resolveRuntimeKeyboardInset({ height: 620, offsetTop: 24 }, 844)
    ).toBe(200);
  });
});

describe("setupRuntimeViewportVarSync", () => {
  it("writes CSS vars once for unchanged viewport measurements", () => {
    const viewport = new MockEventTarget() as MockEventTarget & {
      height: number;
      width: number;
      offsetTop: number;
    };
    viewport.height = 700;
    viewport.width = 390;
    viewport.offsetTop = 0;

    const runtimeWindow = new MockEventTarget() as unknown as MockEventTarget & {
      innerHeight: number;
      innerWidth: number;
      visualViewport: VisualViewport;
      requestAnimationFrame: (cb: (timestamp: number) => void) => number;
      cancelAnimationFrame: (id: number) => void;
    };
    runtimeWindow.innerHeight = 844;
    runtimeWindow.innerWidth = 390;
    runtimeWindow.visualViewport = viewport as unknown as VisualViewport;
    runtimeWindow.requestAnimationFrame = (cb) => {
      cb(0);
      return 1;
    };
    runtimeWindow.cancelAnimationFrame = vi.fn();

    const styleTarget = createStyleTarget();
    const cleanup = setupRuntimeViewportVarSync(runtimeWindow, styleTarget.target);

    expect(styleTarget.writes).toEqual([
      ["--agenthub-vh", "700px"],
      ["--agenthub-vw", "390px"],
      ["--agenthub-keyboard-inset", "144px"],
    ]);

    styleTarget.writes.length = 0;
    viewport.emit("resize");
    runtimeWindow.emit("resize");

    expect(styleTarget.writes).toEqual([]);
    cleanup();
  });
});
