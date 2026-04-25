// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OutputErrorBoundary } from "./components/output_error_boundary";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "./test_utils/react_test_helpers";

installReactDomTestGlobals();

const ThrowOnRender: React.FC = () => {
  throw new Error("render failed");
};

describe("OutputErrorBoundary", () => {
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

  it("renders fallback UI and retries after reset", () => {
    const onReset = vi.fn();

    act(() => {
      renderWithMantine(
        root,
        <OutputErrorBoundary onReset={onReset}>
          {React.createElement(ThrowOnRender)}
        </OutputErrorBoundary>
      );
    });

    expect(container.textContent).toContain("Output failed to render");
    const retryButton = Array.from(container.querySelectorAll("button")).find((node) =>
      node.textContent?.includes("Retry")
    ) as HTMLButtonElement | undefined;
    expect(retryButton).toBeDefined();
    if (!retryButton) return;

    act(() => {
      retryButton.click();
    });

    expect(onReset).toHaveBeenCalledTimes(1);
  });
});
