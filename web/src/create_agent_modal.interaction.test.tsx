// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { CreateAgentModal } from "./components/create_agent_modal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type ModalHarnessProps = {
  worktreeMode: "use_existing" | "create_worktree" | "reuse_worktree";
  worktreeError: string | null;
  agentName?: string;
};

function renderHarness(root: Root, props: ModalHarnessProps) {
  act(() => {
    root.render(
      <MantineProvider>
        <CreateAgentModal
          agentName={props.agentName ?? ""}
          setAgentName={() => {}}
          agentWorkdir=""
          setAgentWorkdir={() => {}}
          agentPresetId="codex"
          setAgentPresetId={() => {}}
          worktreeMode={props.worktreeMode}
          setWorktreeMode={() => {}}
          worktreeRepo=""
          setWorktreeRepo={() => {}}
          worktreeRef=""
          setWorktreeRef={() => {}}
          codeMode={false}
          setCodeMode={() => {}}
          worktreeError={props.worktreeError}
          createBusy={false}
          workdirPlaceholder="~/.agenthub/worktrees"
          withinPortal={false}
          onCreateAgent={() => {}}
          onClose={() => {}}
        />
      </MantineProvider>
    );
  });
}

function findButtonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find((node) =>
    node.textContent?.includes(text)
  );
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  return button as HTMLButtonElement;
}

describe("CreateAgentModal interactions", () => {
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
    class ResizeObserverMock {
      observe() {}
      unobserve() {}
      disconnect() {}
    }
    Object.defineProperty(window, "ResizeObserver", {
      configurable: true,
      writable: true,
      value: ResizeObserverMock,
    });
    Object.defineProperty(globalThis, "ResizeObserver", {
      configurable: true,
      writable: true,
      value: ResizeObserverMock,
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

  it("does not force re-open advanced options after manual collapse when expand condition stays true", () => {
    renderHarness(root, {
      worktreeMode: "create_worktree",
      worktreeError: "repo is required",
    });
    expect(container.textContent).toContain("Hide Advanced Options");
    expect(container.textContent).toContain("Worktree repo path");

    const toggleButton = findButtonByText(container, "Hide Advanced Options");
    act(() => {
      toggleButton.dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
    });
    expect(container.textContent).toContain("Show Advanced Options");
    expect(container.textContent).not.toContain("Worktree repo path");

    renderHarness(root, {
      worktreeMode: "create_worktree",
      worktreeError: "repo path is still missing",
      agentName: "agent-a",
    });
    expect(container.textContent).toContain("Show Advanced Options");
    expect(container.textContent).not.toContain("Worktree repo path");
  });
});
