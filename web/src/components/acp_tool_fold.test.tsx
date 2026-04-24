// @vitest-environment jsdom
import React from "react";
import { createRoot, Root } from "react-dom/client";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAutoCollapseToolFoldWhenOutOfView } from "./acp_tool_fold";

class MockIntersectionObserver {
  static roots: Array<Element | Document | null> = [];
  observe = vi.fn();
  disconnect = vi.fn();

  constructor(
    _callback: IntersectionObserverCallback,
    options?: IntersectionObserverInit
  ) {
    MockIntersectionObserver.roots.push(options?.root ?? null);
  }
}

function HookHarness({
  rootElement,
}: {
  rootElement?: HTMLElement | null;
}) {
  const detailsRef = React.useRef<HTMLDetailsElement | null>(null);
  useAutoCollapseToolFoldWhenOutOfView({
    detailsRef,
    enabled: true,
    rootElement,
    onCollapse: () => {},
  });
  return <details ref={detailsRef} />;
}

describe("useAutoCollapseToolFoldWhenOutOfView", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    MockIntersectionObserver.roots = [];
    vi.stubGlobal("IntersectionObserver", MockIntersectionObserver);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it("recreates the observer when the provided root element changes", () => {
    const rootA = document.createElement("div");
    const rootB = document.createElement("div");

    act(() => {
      root.render(<HookHarness rootElement={rootA} />);
    });
    act(() => {
      root.render(<HookHarness rootElement={rootB} />);
    });

    expect(MockIntersectionObserver.roots).toEqual([rootA, rootB]);
  });
});
