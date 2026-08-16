// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  installReactDomTestGlobals,
  renderWithMantine,
} from "../test_utils/react_test_helpers";

const {
  joinStart,
  joinFinish,
  registerStart,
  registerFinish,
  acceptTeamspaceInvite,
  ensurePushSubscription,
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
} = vi.hoisted(() => ({
  joinStart: vi.fn(),
  joinFinish: vi.fn(),
  registerStart: vi.fn(),
  registerFinish: vi.fn(),
  acceptTeamspaceInvite: vi.fn(),
  ensurePushSubscription: vi.fn().mockResolvedValue(undefined),
  publicKeyCredentialCreationOptionsFromJson: vi.fn(),
  registerCredentialToJson: vi.fn(),
}));

vi.mock("../api", () => ({
  api: {
    joinStart,
    joinFinish,
    registerStart,
    registerFinish,
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
  publicKeyCredentialCreationOptionsFromJson,
  registerCredentialToJson,
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

function setInputValue(container: HTMLElement, id: string, value: string) {
  const input = container.querySelector<HTMLInputElement>(`#${id}`);
  if (!input) {
    throw new Error(`input not found: ${id}`);
  }
  act(() => {
    const valueSetter = Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value"
    )?.set;
    valueSetter?.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

describe("JoinPage error handling", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    joinStart.mockReset();
    joinFinish.mockReset();
    registerStart.mockReset();
    registerFinish.mockReset();
    acceptTeamspaceInvite.mockReset();
    ensurePushSubscription.mockClear();
    publicKeyCredentialCreationOptionsFromJson.mockReset();
    registerCredentialToJson.mockReset();
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

  it("finishes Teamspace registration through WebAuthn when required", async () => {
    const onComplete = vi.fn();
    const credential = {} as PublicKeyCredential;
    const createCredential = vi.fn().mockResolvedValue(credential);
    Object.defineProperty(navigator, "credentials", {
      configurable: true,
      value: { create: createCredential },
    });
    window.history.pushState({}, "", "/join#teamspace-token");
    registerStart.mockResolvedValueOnce({
      challenge_id: "challenge-1",
      options: { publicKey: {} },
    });
    publicKeyCredentialCreationOptionsFromJson.mockReturnValueOnce({});
    registerCredentialToJson.mockReturnValueOnce({ id: "credential-1" });
    registerFinish.mockResolvedValueOnce({
      user_id: "team-user-id",
      token: "team-session-token",
      role: "contributor",
    });

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={onComplete} />);
      await Promise.resolve();
    });

    setInputValue(container, "teamspace-username", "new-user");
    setInputValue(container, "teamspace-display-name", "New User");
    setInputValue(container, "teamspace-password", "secret");
    await act(async () => {
      clickByText(container, "Join Teamspace");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(registerFinish).toHaveBeenCalledWith("challenge-1", { id: "credential-1" });
    expect(onComplete).toHaveBeenCalledWith({
      userId: "team-user-id",
      token: "team-session-token",
      username: "new-user",
      role: "contributor",
    });
  });

  it("disables the submit button while a Teamspace invite accept is in flight, preventing a duplicate request", async () => {
    window.history.pushState({}, "", "/join#teamspace-token");
    let resolveAccept: (() => void) | undefined;
    acceptTeamspaceInvite.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          resolveAccept = resolve;
        })
    );

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

    const findButton = () =>
      Array.from(container.querySelectorAll("button")).find((node) =>
        node.textContent?.includes("Join as existing")
      ) ?? Array.from(container.querySelectorAll("button")).find((node) =>
        node.textContent?.includes("Joining...")
      );

    await act(async () => {
      findButton()?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    const busyButton = findButton();
    expect(busyButton?.textContent).toContain("Joining...");
    expect(busyButton?.hasAttribute("disabled")).toBe(true);

    // A second click while busy must not fire a second request.
    act(() => {
      busyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(acceptTeamspaceInvite).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveAccept?.();
      await Promise.resolve();
    });
  });

  it("disables the Join Device submit button while joining is in flight, preventing a duplicate request", async () => {
    let resolveJoin: (() => void) | undefined;
    joinStart.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveJoin = () => resolve({ user_id: "test-user-id", token: "session-token" });
        })
    );

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={() => {}} />);
      await Promise.resolve();
    });

    await act(async () => {
      clickByText(container, "Join");
      await Promise.resolve();
    });

    const busyButton = Array.from(container.querySelectorAll("button")).find((node) =>
      node.textContent?.includes("Joining...")
    );
    expect(busyButton?.hasAttribute("disabled")).toBe(true);

    act(() => {
      busyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(joinStart).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveJoin?.();
      await Promise.resolve();
      await Promise.resolve();
    });
  });

  it("disables the Join Teamspace submit button while registering is in flight, preventing a duplicate request", async () => {
    window.history.pushState({}, "", "/join#teamspace-token");
    let resolveRegister: (() => void) | undefined;
    registerStart.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveRegister = () =>
            resolve({ user_id: "team-user-id", token: "team-session-token", role: "operator" });
        })
    );

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={() => {}} />);
      await Promise.resolve();
    });

    await act(async () => {
      clickByText(container, "Join Teamspace");
      await Promise.resolve();
    });

    const busyButton = Array.from(container.querySelectorAll("button")).find((node) =>
      node.textContent?.includes("Joining...")
    );
    expect(busyButton?.hasAttribute("disabled")).toBe(true);

    act(() => {
      busyButton?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    expect(registerStart).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveRegister?.();
      await Promise.resolve();
      await Promise.resolve();
    });
  });

  it("shows a normalized error when accepting a Teamspace invite fails", async () => {
    window.history.pushState({}, "", "/join#teamspace-token");
    acceptTeamspaceInvite.mockRejectedValueOnce(new Error('{"error":"invite expired"}'));

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

    await act(async () => {
      clickByText(container, "Join as existing");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain("invite expired");
  });

  it("shows a normalized error when Teamspace registration fails", async () => {
    window.history.pushState({}, "", "/join#teamspace-token");
    registerStart.mockRejectedValueOnce(new Error('{"error":"invite expired"}'));

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={() => {}} />);
      await Promise.resolve();
    });

    await act(async () => {
      clickByText(container, "Join Teamspace");
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(container.textContent).toContain("invite expired");
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

  it("falls back to the device join form when the invite fragment is malformed", async () => {
    window.history.pushState({}, "", "/join?token=test-token#%E0%A4%A");

    await act(async () => {
      renderWithMantine(root, <JoinPage auth={null} onComplete={() => {}} />);
      await Promise.resolve();
    });

    expect(container.textContent).toContain("Join Device");
  });
});
