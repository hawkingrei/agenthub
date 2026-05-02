// @vitest-environment jsdom
import { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { AdminPage, type AdminPageProps } from "./admin_page";
import type { AuthState } from "../types";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia;
}

function required<T>(value: T | null | undefined, message: string): T {
  if (value == null) {
    throw new Error(message);
  }
  return value;
}

const rootAuth: AuthState = {
  token: "token-1",
  userId: "root-user",
  username: "root",
  role: "root",
};

function createAdminPageProps(
  overrides: Partial<AdminPageProps> = {}
): AdminPageProps {
  return {
    auth: rootAuth,
    error: null,
    setError: () => {},
    safePaths: {
      safePaths: [],
      selectedSafePaths: new Set<string>(),
      safePathInput: "",
      setSafePathInput: () => {},
      onAddSafePath: () => {},
      onToggleSafePath: () => {},
      onToggleAllSafePaths: () => {},
      onDeleteSelectedSafePaths: () => {},
      onDeleteSafePath: () => {},
    },
    devices: {
      devices: [],
      onRevokeDevice: () => {},
    },
    audits: {
      audits: [],
    },
    join: {
      onCreateJoin: () => {},
      joinUrl: null,
      joinToken: null,
      joinPin: null,
    },
    vapid: {
      vapidInfo: null,
      onRotateVapid: () => {},
    },
    ui: {
      developerMode: false,
      onDeveloperModeChange: () => {},
    },
    system: {
      passkeyEnabled: false,
      onPasskeyEnabledChange: () => {},
    },
    ...overrides,
  };
}

describe("AdminPage", () => {
  let container: HTMLDivElement;
  let root: Root;
  let originalClipboard: Clipboard | undefined;
  let hadClipboard = false;

  beforeEach(() => {
    hadClipboard = "clipboard" in navigator;
    originalClipboard = navigator.clipboard;
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
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
    if (hadClipboard) {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: originalClipboard,
      });
    } else {
      Reflect.deleteProperty(navigator, "clipboard");
    }
  });

  it("exposes browser-local developer mode controls in UI tab", () => {
    const onDeveloperModeChange = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <AdminPage
            {...createAdminPageProps({
              ui: {
                developerMode: false,
                onDeveloperModeChange,
              },
            })}
          />
        </MantineProvider>
      );
    });

    const uiTab = Array.from(container.querySelectorAll("button")).find((button) =>
      button.textContent?.includes("UI")
    );
    required(uiTab, "ui tab missing");
    act(() => {
      uiTab?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(container.textContent).toContain("Developer Mode");
    expect(container.textContent).toContain(
      "Applies to this browser only. Affects Agents and Teams."
    );

    const toggle = required(
      container.querySelector('input[type="checkbox"]') as HTMLInputElement | null,
      "developer mode toggle missing"
    );
    act(() => {
      toggle.click();
    });

    expect(onDeveloperModeChange).toHaveBeenCalledWith(true);
  });

  it("renders system config tab and handles passkey toggle", () => {
    const onPasskeyEnabledChange = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <AdminPage
            {...createAdminPageProps({
              system: {
                passkeyEnabled: null,
                onPasskeyEnabledChange,
              },
            })}
          />
        </MantineProvider>
      );
    });

    const systemTab = required(
      Array.from(container.querySelectorAll("button")).find((button) =>
        button.textContent?.includes("System")
      ),
      "system tab missing"
    );
    act(() => {
      systemTab.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(container.textContent).toContain("System Configuration");
    const toggle = required(
      container.querySelector('input[type="checkbox"]') as HTMLInputElement | null,
      "passkey toggle missing"
    );
    // When null, it should be disabled and show loading text
    expect(toggle.disabled).toBe(true);
    expect(container.textContent).toContain("Loading configuration...");

    // Re-render with enabled=false
    act(() => {
      root.render(
        <MantineProvider>
          <AdminPage
            {...createAdminPageProps({
              system: {
                passkeyEnabled: false,
                onPasskeyEnabledChange,
              },
            })}
          />
        </MantineProvider>
      );
    });

    expect(toggle.disabled).toBe(false);
    expect(toggle.checked).toBe(false);
    act(() => {
      toggle.click();
    });
    expect(onPasskeyEnabledChange).toHaveBeenCalledWith(true);
  });

  it("shows token-based join guidance instead of a QR code", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <AdminPage
            {...createAdminPageProps({
              join: {
                onCreateJoin: () => {},
                joinUrl: "https://agenthub.example.com/join?token=abc",
                joinToken: "abc",
                joinPin: "123456",
              },
            })}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Create Join Token");
    const joinTab = required(
      Array.from(container.querySelectorAll("button")).find((button) =>
        button.textContent?.includes("Join Device")
      ),
      "join tab missing"
    );
    act(() => {
      joinTab.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });

    expect(container.textContent).toContain(
      "Use the token/link below on the destination browser. QR onboarding is no longer required."
    );
    expect(container.textContent).toContain(
      "Join link: https://agenthub.example.com/join?token=abc"
    );
    expect(container.textContent).toContain("Copy link");
    expect(container.querySelector("img")).toBeNull();
  });
});
