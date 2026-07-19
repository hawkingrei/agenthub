// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { WorkbenchHeaderMenu } from "./components/workbench_header_menu";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function findButtonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find((node) =>
    node.textContent?.includes(text)
  );
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  return button as HTMLButtonElement;
}

describe("WorkbenchHeaderMenu interactions", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: () => {},
        removeListener: () => {},
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => false,
      })),
    });
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

  it("routes intra-app menu actions through the SPA navigate callback", () => {
    const onNavigate = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <WorkbenchHeaderMenu
            active="teams"
            username="root"
            isRoot={true}
            onLogout={() => {}}
            onNavigate={onNavigate}
            buttonClassName="menu-button"
            defaultOpened={true}
          />
        </MantineProvider>
      );
    });

    act(() => {
      findButtonByText(container, "Workspace").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });
    act(() => {
      findButtonByText(container, "Nodes").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });
    act(() => {
      findButtonByText(container, "Settings").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });

    expect(onNavigate).toHaveBeenNthCalledWith(1, "/workspace");
    expect(onNavigate).toHaveBeenNthCalledWith(2, "/workspace/nodes");
    expect(onNavigate).toHaveBeenNthCalledWith(3, "/admin");
  });

  it("routes the Teams menu item through the canonical selector path", () => {
    const onNavigate = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <WorkbenchHeaderMenu
            active="workspace"
            username="root"
            isRoot={false}
            onLogout={() => {}}
            onNavigate={onNavigate}
            buttonClassName="menu-button"
            defaultOpened={true}
          />
        </MantineProvider>
      );
    });

    act(() => {
      findButtonByText(container, "Teams").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });

    expect(onNavigate).toHaveBeenCalledWith("/workspace/teams");
  });
});
