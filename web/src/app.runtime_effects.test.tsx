// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { authStatusMock, loginStartMock } = vi.hoisted(() => ({
  authStatusMock: vi.fn(),
  loginStartMock: vi.fn(),
}));

vi.mock("./api", () => ({
  api: {
    authStatus: authStatusMock,
    loginStart: loginStartMock,
  },
  parseApiErrorMessage: vi.fn(() => null),
}));

vi.mock("./push", () => ({
  ensurePushSubscription: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("./webauthn", () => ({
  loginCredentialToJson: vi.fn(() => ({})),
  publicKeyCredentialCreationOptionsFromJson: vi.fn((options) => options),
  publicKeyCredentialRequestOptionsFromJson: vi.fn((options) => options),
  registerCredentialToJson: vi.fn(() => ({})),
}));

import { App } from "./app";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

class MockVisualViewport extends EventTarget {
  width: number;
  height: number;
  offsetTop: number;

  constructor(width: number, height: number) {
    super();
    this.width = width;
    this.height = height;
    this.offsetTop = 0;
  }
}

describe("App runtime viewport effects", () => {
  let container: HTMLDivElement;
  let root: Root;
  let mockViewport: MockVisualViewport;
  let credentialsGetMock: ReturnType<typeof vi.fn>;
  let credentialsCreateMock: ReturnType<typeof vi.fn>;
  let originalCredentialsDescriptor: PropertyDescriptor | undefined;

  beforeEach(() => {
    authStatusMock.mockReset();
    authStatusMock.mockResolvedValue({ root_initialized: true });
    loginStartMock.mockReset();
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
    originalCredentialsDescriptor = Object.getOwnPropertyDescriptor(
      globalThis.navigator,
      "credentials"
    );
    credentialsGetMock = vi.fn();
    credentialsCreateMock = vi.fn();
    Object.defineProperty(globalThis.navigator, "credentials", {
      configurable: true,
      value: {
        get: credentialsGetMock,
        create: credentialsCreateMock,
      },
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
    document.documentElement.style.removeProperty("--agenthub-keyboard-inset");
    if (originalCredentialsDescriptor) {
      Object.defineProperty(
        globalThis.navigator,
        "credentials",
        originalCredentialsDescriptor
      );
    } else {
      delete (globalThis.navigator as Navigator & { credentials?: unknown }).credentials;
    }
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

  it("accounts for visual viewport offsetTop when syncing viewport height", async () => {
    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    mockViewport.height = 650;
    mockViewport.offsetTop = 24;
    await act(async () => {
      mockViewport.dispatchEvent(new Event("resize"));
      await Promise.resolve();
    });

    expect(
      document.documentElement.style.getPropertyValue("--agenthub-vh")
    ).toBe("674px");
    expect(
      document.documentElement.style.getPropertyValue("--agenthub-keyboard-inset")
    ).toBe("26px");
  });

  it("serializes login attempts while a WebAuthn request is pending", async () => {
    loginStartMock.mockResolvedValue({
      challenge_id: "challenge-1",
      options: {},
    });
    let resolveCredential: ((value: null) => void) | null = null;
    credentialsGetMock.mockImplementation(
      () =>
        new Promise<null>((resolve) => {
          resolveCredential = resolve;
        })
    );

    await act(async () => {
      root.render(<App />);
      await Promise.resolve();
    });

    const usernameInput = container.querySelector(
      'input[placeholder="Username"]'
    ) as HTMLInputElement | null;
    const passwordInput = container.querySelector(
      'input[placeholder="Password"]'
    ) as HTMLInputElement | null;
    const loginButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent?.includes("Login")
    ) as HTMLButtonElement | undefined;

    expect(usernameInput).toBeTruthy();
    expect(passwordInput).toBeTruthy();
    expect(loginButton).toBeTruthy();

    await act(async () => {
      usernameInput!.value = "root";
      usernameInput!.dispatchEvent(new Event("input", { bubbles: true }));
      passwordInput!.value = "secret";
      passwordInput!.dispatchEvent(new Event("input", { bubbles: true }));
      await Promise.resolve();
    });

    await act(async () => {
      loginButton!.click();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(loginStartMock).toHaveBeenCalledTimes(1);
    expect(credentialsGetMock).toHaveBeenCalledTimes(1);
    expect(loginButton!.disabled).toBe(true);
    expect(loginButton!.textContent).toContain("Logging in...");

    await act(async () => {
      loginButton!.click();
      await Promise.resolve();
    });

    expect(loginStartMock).toHaveBeenCalledTimes(1);
    expect(credentialsGetMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveCredential?.(null);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(loginButton!.disabled).toBe(false);
    expect(loginButton!.textContent).toContain("Login");
  });
});
