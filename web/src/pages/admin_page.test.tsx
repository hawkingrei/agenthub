// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import { AdminPage } from "./admin_page";

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

describe("AdminPage", () => {
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

  it("exposes browser-local developer mode controls in UI tab", () => {
    const onDeveloperModeChange = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <AdminPage
            auth={{ username: "root", role: "root" }}
            error={null}
            setError={() => {}}
            safePaths={[]}
            selectedSafePaths={new Set<string>()}
            onToggleSafePath={() => {}}
            onToggleAllSafePaths={() => {}}
            onDeleteSelectedSafePaths={() => {}}
            devices={[]}
            audits={[]}
            vapidInfo={null}
            onRotateVapid={() => {}}
            onAddSafePath={() => {}}
            onDeleteSafePath={() => {}}
            onRevokeDevice={() => {}}
            onCreateJoin={() => {}}
            joinQr={null}
            joinToken={null}
            joinPin={null}
            safePathInput=""
            setSafePathInput={() => {}}
            developerMode={false}
            onDeveloperModeChange={onDeveloperModeChange}
            passkeyEnabled={false}
            onPasskeyEnabledChange={() => {}}
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
});
