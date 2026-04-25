// @vitest-environment jsdom
import { createRoot, type Root } from "react-dom/client";
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  TeamPanelLoadingFallback,
  prefetchTeamSetupSurface,
  prefetchTeamWorkbenchTab,
} from "./team_workbench_content";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("team_workbench_content", () => {
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
  });

  it("renders the shared loading fallback chrome", () => {
    act(() => {
      root.render(<TeamPanelLoadingFallback />);
    });

    expect(container.textContent).toContain("Loading panel...");
    expect(container.firstElementChild?.className).toContain("rounded-2xl");
  });

  it("prefetches each lazy workbench surface without throwing", async () => {
    expect(() => prefetchTeamWorkbenchTab("runs")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("overview")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("events")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("steps")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("mailbox")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("member_console")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("conversation")).not.toThrow();
    expect(() => prefetchTeamSetupSurface()).not.toThrow();

    await act(async () => {
      await vi.dynamicImportSettled();
    });
  });
});
