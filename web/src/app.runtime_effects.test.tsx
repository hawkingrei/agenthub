// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({
  api: {
    authStatus: vi.fn().mockResolvedValue({ root_initialized: true }),
  },
}));

import { App } from "./app";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

class MockVisualViewport extends EventTarget {
  width: number;
  height: number;

  constructor(width: number, height: number) {
    super();
    this.width = width;
    this.height = height;
  }
}

describe("App runtime viewport effects", () => {
  let container: HTMLDivElement;
  let root: Root;
  let mockViewport: MockVisualViewport;

  beforeEach(() => {
    if (
      typeof globalThis.localStorage !== "undefined" &&
      typeof globalThis.localStorage.clear === "function"
    ) {
      globalThis.localStorage.clear();
    }
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    mockViewport = new MockVisualViewport(390, 700);
    Object.defineProperty(window, "visualViewport", {
      configurable: true,
      value: mockViewport,
    });
    Object.defineProperty(window, "innerWidth", {
      configurable: true,
      value: 390,
    });
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 700,
    });
    Object.defineProperty(window, "requestAnimationFrame", {
      configurable: true,
      value: undefined,
    });
    Object.defineProperty(window, "cancelAnimationFrame", {
      configurable: true,
      value: undefined,
    });
  });

  afterEach(() => {
    if (root) {
      act(() => {
        root.unmount();
      });
    }
    if (container) {
      container.remove();
    }
    document.documentElement.style.removeProperty("--agenthub-vh");
    document.documentElement.style.removeProperty("--agenthub-vw");
    document.documentElement.style.removeProperty("--agenthub-safe-bottom");
    vi.restoreAllMocks();
  });

  it("renders login shell and syncs runtime viewport css vars", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain("AgentHub");
    expect(container.textContent).toContain("Password + Passkey Login");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("700px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("390px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-safe-bottom")
    ).toBe("env(safe-area-inset-bottom, 0px)");

    mockViewport.height = 666;
    mockViewport.width = 360;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });

    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("666px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("360px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-safe-bottom")
    ).toBe("env(safe-area-inset-bottom, 0px)");
  });

  it("suppresses bottom safe inset while keyboard overlap is large", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    mockViewport.height = 500;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });

    expect(
      document.documentElement.style.getPropertyValue("--agenthub-safe-bottom")
    ).toBe("0px");

    mockViewport.height = 680;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });

    expect(
      document.documentElement.style.getPropertyValue("--agenthub-safe-bottom")
    ).toBe("env(safe-area-inset-bottom, 0px)");
  });

  it("does not collapse runtime viewport vars when visual viewport reports tiny transient values", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    mockViewport.height = 1;
    mockViewport.width = 1;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });

    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("700px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("390px");
  });

  it("ignores non-finite viewport anomalies during keyboard transitions", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    mockViewport.height = Number.NaN;
    mockViewport.width = Number.POSITIVE_INFINITY;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("700px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("390px");

    mockViewport.height = Number.NEGATIVE_INFINITY;
    mockViewport.width = Number.NaN;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("scroll"));
      await Promise.resolve();
    });
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("700px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("390px");
  });

  it("recovers viewport vars once keyboard transition returns to valid dimensions", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    mockViewport.height = 0;
    mockViewport.width = 1;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("700px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("390px");

    mockViewport.height = 644;
    mockViewport.width = 358;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("644px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vw")
    ).toBe("358px");
  });
});
