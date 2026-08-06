// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "../test_utils/react_test_helpers";

const { joinStart, joinFinish, registerStart, acceptTeamspaceInvite, ensurePushSubscription } = vi.hoisted(() => ({
  joinStart: vi.fn(),
  joinFinish: vi.fn(),
  registerStart: vi.fn(),
  acceptTeamspaceInvite: vi.fn(),
  ensurePushSubscription: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../api", () => ({
  api: {
    joinStart,
    joinFinish,
    registerStart,
    acceptTeamspaceInvite,
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
  stringifyApiError: (err: unknown) => {
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
    return String(err);
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

installReactDomTestGlobals();

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
    registerStart.mockReset();
    acceptTeamspaceInvite.mockReset();
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
      renderWithMantine(root, <JoinPage auth={null} onComplete={() => {}} />);
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

  it("completes joining without passkey challenge when token is returned in start response", async () => {
    const onComplete = vi.fn();
    joinStart.mockResolvedValueOnce({
      user_id: "test-user-id",
      token: "session-token",
    });

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={onComplete} />);
      await Promise.resolve();
    });

    await act(async () => {
      clickByText(container, "Join");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(onComplete).toHaveBeenCalledWith({
      userId: "test-user-id",
      token: "session-token",
      username: "", // initial state
      role: "device",
    });
    expect(ensurePushSubscription).toHaveBeenCalledWith("session-token");
    expect(joinFinish).not.toHaveBeenCalled();
  });

  it("clears a Teamspace invite fragment and binds it during local registration", async () => {
    const onComplete = vi.fn();
    window.history.pushState({}, "", "/join#teamspace-token");
    registerStart.mockResolvedValueOnce({
      user_id: "team-user-id",
      token: "team-session-token",
      role: "operator",
    });

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={onComplete} />);
      await Promise.resolve();
    });

    expect(window.location.hash).toBe("");
    expect(container.textContent).toContain("Join Teamspace");
    await act(async () => {
      clickByText(container, "Join Teamspace");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(registerStart).toHaveBeenCalledWith(
      "",
      "",
      undefined,
      "",
      undefined,
      "teamspace-token"
    );
    expect(onComplete).toHaveBeenCalledWith({
      userId: "team-user-id",
      token: "team-session-token",
      username: "",
      role: "operator",
    });
  });

  it("accepts a Teamspace invite with an existing local account", async () => {
    window.history.pushState({}, "", "/join#teamspace-token");
    acceptTeamspaceInvite.mockResolvedValueOnce({
      team_id: "team-1",
      user_id: "existing-user",
      role: "contributor",
    });

    await act(async () => {
      renderWithMantine(
        root,
        <JoinPage
          auth={{
            token: "existing-session",
            userId: "existing-user",
            username: "existing",
            role: "operator",
          }}
          onComplete={() => {}}
        />
      );
      await Promise.resolve();
    });

    expect(container.textContent).toContain("Join as existing");
    await act(async () => {
      clickByText(container, "Join as existing");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(acceptTeamspaceInvite).toHaveBeenCalledWith("existing-session", "teamspace-token");
  });
});
