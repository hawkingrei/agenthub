// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { joinStart, joinFinish, ensurePushSubscription } = vi.hoisted(() => ({
  joinStart: vi.fn(),
  joinFinish: vi.fn(),
  ensurePushSubscription: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../api", () => ({
  api: {
    joinStart,
    joinFinish,
  },
  parseApiErrorMessage: (err: unknown) => {
    if (err instanceof Error && err.message.trim().startsWith("{")) {
      try {
        const parsed = JSON.parse(err.message) as { error?: string };
        if (typeof parsed.error === "string" && parsed.error) {
          return parsed.error;
        }
      } catch {
        return err.message;
      }
    }
    if (err instanceof Error) {
      return err.message;
    }
    return null;
  },
}));

vi.mock("../push", () => ({
  ensurePushSubscription,
}));

vi.mock("../webauthn", () => ({
  publicKeyCredentialCreationOptionsFromJson: vi.fn(),
  registerCredentialToJson: vi.fn(),
}));

vi.mock("../storage/safe_storage", () => ({
  setLocalStorageItemSafe: vi.fn(),
}));

import { JoinPage } from "./join_page";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function clickByText(container: HTMLElement, text: string) {
  const button = Array.from(container.querySelectorAll("button")).find((node) =>
    node.textContent?.includes(text)
  );
  if (!button) {
    throw new Error(`button not found: ${text}`);
  }
  act(() => {
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

describe("JoinPage error handling", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    joinStart.mockReset();
    joinFinish.mockReset();
    ensurePushSubscription.mockClear();
    window.history.pushState({}, "", "/join?token=test-token");

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

  it("renders normalized plain error message when API throws JSON error payload", async () => {
    joinStart.mockRejectedValueOnce(new Error("{\"error\":\"user not found\"}"));

    await act(async () => {
      root.render(<JoinPage onComplete={() => {}} />);
      await Promise.resolve();
    });

    await act(async () => {
      clickByText(container, "Join");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain("user not found");
    expect(container.textContent).not.toContain("{\"error\":\"user not found\"}");
    expect(joinStart).toHaveBeenCalledTimes(1);
  });
});
