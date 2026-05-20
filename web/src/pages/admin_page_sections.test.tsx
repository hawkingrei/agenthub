// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  AdminAuditsSection,
  AdminDevicesSection,
  AdminJoinSection,
  AdminLinkersSection,
  AdminSafePathsSection,
  AdminSystemSection,
  AdminUiSection,
  AdminVapidSection,
} from "./admin_page_sections";
import {
  installReactDomTestGlobals,
  renderWithMantine,
  required,
} from "../test_utils/react_test_helpers";

installReactDomTestGlobals();

describe("admin_page_sections", () => {
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

  it("wires safe-path section interactions through the section contract", () => {
    const setSafePathInput = vi.fn();
    const onAddSafePath = vi.fn();
    const onToggleAllSafePaths = vi.fn();
    const onDeleteSelectedSafePaths = vi.fn();
    const onToggleSafePath = vi.fn();
    const onDeleteSafePath = vi.fn();

    renderWithMantine(
      root,
      <AdminSafePathsSection
        safePaths={{
          safePaths: [{ path: "/repo", created_at: 1 }],
          selectedSafePaths: new Set<string>(["/repo"]),
          safePathInput: "/tmp/new",
          setSafePathInput,
          onAddSafePath,
          onToggleSafePath,
          onToggleAllSafePaths,
          onDeleteSelectedSafePaths,
          onDeleteSafePath,
        }}
      />
    );

    const textInput = required(
      container.querySelector('input[placeholder="Add safe path"]') as HTMLInputElement | null,
      "safe path input missing"
    );
    act(() => {
      const descriptor = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value"
      );
      descriptor?.set?.call(textInput, "/tmp/updated");
      textInput.dispatchEvent(new Event("input", { bubbles: true }));
      textInput.dispatchEvent(new Event("change", { bubbles: true }));
    });

    const buttons = Array.from(container.querySelectorAll("button"));
    const addPathButton = buttons.find((button) => button.textContent === "Add Path");
    const deleteSelectedButton = buttons.find(
      (button) => button.textContent === "Delete Selected"
    );
    const deleteButton = buttons.find((button) => button.textContent === "Delete");
    const checkboxes = Array.from(
      container.querySelectorAll('input[type="checkbox"]')
    ) as HTMLInputElement[];
    const selectAll = checkboxes[0];
    const itemToggle = checkboxes[1];

    act(() => {
      addPathButton?.click();
      selectAll?.click();
      deleteSelectedButton?.click();
      itemToggle?.click();
      deleteButton?.click();
    });

    expect(setSafePathInput).toHaveBeenCalledWith("/tmp/updated");
    expect(onAddSafePath).toHaveBeenCalledTimes(1);
    expect(onToggleAllSafePaths).toHaveBeenCalledTimes(1);
    expect(onDeleteSelectedSafePaths).toHaveBeenCalledTimes(1);
    expect(onToggleSafePath).toHaveBeenCalledWith("/repo");
    expect(onDeleteSafePath).toHaveBeenCalledWith("/repo");
  });

  it("renders join, vapid, devices, and audits sections with their section data", () => {
    const onCopyJoinLink = vi.fn();
    const onRotateVapid = vi.fn();
    const onRevokeDevice = vi.fn();

    renderWithMantine(
      root,
      <>
        <AdminJoinSection
          join={{
            onCreateJoin: vi.fn(),
            joinUrl: "https://agenthub.example.com/join?token=abc",
            joinToken: "abc",
            joinPin: "123456",
          }}
          joinLinkCopyState="idle"
          onCopyJoinLink={onCopyJoinLink}
        />
        <AdminVapidSection
          vapid={{
            vapidInfo: {
              subject: "mailto:test@example.com",
              public_key: "public-key",
              keys_path: "/tmp/vapid.json",
            },
            onRotateVapid,
          }}
        />
        <AdminDevicesSection
          devices={{
            devices: [
              { id: "device-1", name: "MacBook", status: "active" },
              { id: "device-2", name: "iPhone", status: "revoked" },
            ] as Parameters<typeof AdminDevicesSection>[0]["devices"]["devices"],
            onRevokeDevice,
          }}
        />
        <AdminAuditsSection
          audits={{
            audits: [
              {
                id: "audit-1",
                ts: 1,
                event: "login",
                user_id: "root",
                device_id: "device-1",
                ip: "127.0.0.1",
                user_agent: "agenthub-test",
                detail: null,
              },
            ] as unknown as Parameters<
              typeof AdminAuditsSection
            >[0]["audits"]["audits"],
          }}
        />
      </>
    );

    expect(container.textContent).toContain("Join link: https://agenthub.example.com/join?token=abc");
    expect(container.textContent).toContain("Token: abc");
    expect(container.textContent).toContain("PIN: 123456");
    expect(container.textContent).toContain("mailto:test@example.com");
    expect(container.textContent).toContain("public-key");
    expect(container.textContent).toContain("/tmp/vapid.json");
    expect(container.textContent).toContain("MacBook - active");
    expect(container.textContent).toContain("iPhone - revoked");
    expect(container.textContent).toContain("login");

    const copyButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Copy link"
    );
    const rotateButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Rotate Keys"
    );
    const revokeButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Revoke"
    );

    act(() => {
      copyButton?.click();
      rotateButton?.click();
      revokeButton?.click();
    });

    expect(onCopyJoinLink).toHaveBeenCalledTimes(1);
    expect(onRotateVapid).toHaveBeenCalledTimes(1);
    expect(onRevokeDevice).toHaveBeenCalledWith("device-1");
  });

  it("renders empty vapid state and system/ui toggles from section props", () => {
    const onDeveloperModeChange = vi.fn();
    const onPasskeyEnabledChange = vi.fn();

    renderWithMantine(
      root,
      <>
        <AdminVapidSection
          vapid={{
            vapidInfo: null,
            onRotateVapid: vi.fn(),
          }}
        />
        <AdminUiSection
          ui={{
            developerMode: false,
            onDeveloperModeChange,
          }}
        />
        <AdminSystemSection
          system={{
            passkeyEnabled: false,
            onPasskeyEnabledChange,
          }}
        />
      </>
    );

    expect(container.textContent).toContain("VAPID keys not loaded.");
    expect(container.textContent).toContain("Developer Mode");
    expect(container.textContent).toContain("Enable Passkey");

    const checkboxes = Array.from(
      container.querySelectorAll('input[type="checkbox"]')
    ) as HTMLInputElement[];

    act(() => {
      checkboxes[0]?.click();
      checkboxes[1]?.click();
    });

    expect(onDeveloperModeChange).toHaveBeenCalledWith(true);
    expect(onPasskeyEnabledChange).toHaveBeenCalledWith(true);
  });

  it("renders Slock linker section and wires actions", () => {
    const setSlockApiOrigin = vi.fn();
    const setSlockClientId = vi.fn();
    const setSlockClientSecret = vi.fn();
    const setSlockReturnUrl = vi.fn();
    const setSlockScopesInput = vi.fn();
    const setSlockCallbackInput = vi.fn();
    const onSaveSlockLinker = vi.fn();
    const onCreateSlockLinkAttempt = vi.fn();
    const onExchangeSlockCode = vi.fn();

    renderWithMantine(
      root,
      <AdminLinkersSection
        linkers={{
          slockLinker: {
            linker_id: "slock-primary",
            connector_id: "slock",
            display_name: "Slock",
            status: "connected",
            api_origin: "https://api.slock.ai",
            client_id: "agenthub",
            return_url: "https://agenthub.example.com/api/linkers/slock/callback",
            scopes: ["identity", "openid", "profile"],
            client_secret_configured: true,
            token_configured: true,
            token_type: "Bearer",
            granted_scopes: ["identity", "openid", "profile"],
            expires_at: 100,
            updated_at: 1,
            principal: {
              subject: "slock-agent-1",
              principal_type: "agent",
              display_name: "Claude Assistant",
              handle: "assistant",
              avatar_url: null,
              server_id: "server-1",
              server_slug: "dev",
              updated_at: 1,
            },
          },
          slockLinkAttempt: null,
          slockApiOrigin: "https://api.slock.ai",
          setSlockApiOrigin,
          slockClientId: "agenthub",
          setSlockClientId,
          slockClientSecret: "",
          setSlockClientSecret,
          slockReturnUrl: "https://agenthub.example.com/api/linkers/slock/callback",
          setSlockReturnUrl,
          slockScopesInput: "identity openid profile",
          setSlockScopesInput,
          slockCallbackInput: "callback-code",
          setSlockCallbackInput,
          onSaveSlockLinker,
          onCreateSlockLinkAttempt,
          onExchangeSlockCode,
        }}
      />
    );

    expect(container.textContent).toContain("Slock Linker");
    expect(container.textContent).toContain("connected");
    expect(container.textContent).toContain("Claude Assistant");
    expect(container.textContent).toContain("agent");
    expect(container.textContent).toContain("slock-agent-1");
    expect(container.querySelector('img[src="/slock-icon.png"]')).not.toBeNull();

    const setInputValue = (input: HTMLInputElement, value: string) => {
      const descriptor = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value"
      );
      descriptor?.set?.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
    };
    const findInputByValue = (value: string) =>
      required(
        Array.from(container.querySelectorAll("input")).find(
          (input) => input.value === value
        ) ?? null,
        `input ${value} missing`
      );
    const secretInput = required(
      container.querySelector(
        'input[type="password"]'
      ) as HTMLInputElement | null,
      "client secret input missing"
    );
    const codeInput = findInputByValue("callback-code");
    act(() => {
      setInputValue(
        findInputByValue("https://api.slock.ai"),
        "https://slock.example"
      );
      setInputValue(findInputByValue("agenthub"), "agenthub-next");
      setInputValue(secretInput, "secret-next");
      setInputValue(
        findInputByValue("identity openid profile"),
        "identity profile"
      );
      setInputValue(
        findInputByValue("https://agenthub.example.com/api/linkers/slock/callback"),
        "https://agenthub.example.com/callback"
      );
      setInputValue(codeInput, "next-code");
    });

    const buttons = Array.from(container.querySelectorAll("button"));
    act(() => {
      buttons.find((button) => button.textContent === "Save Slock")?.click();
      buttons.find((button) => button.textContent === "Create Link Attempt")?.click();
      buttons.find((button) => button.textContent === "Exchange Code")?.click();
    });

    expect(setSlockApiOrigin).toHaveBeenCalledWith("https://slock.example");
    expect(setSlockClientId).toHaveBeenCalledWith("agenthub-next");
    expect(setSlockClientSecret).toHaveBeenCalledWith("secret-next");
    expect(setSlockScopesInput).toHaveBeenCalledWith("identity profile");
    expect(setSlockReturnUrl).toHaveBeenCalledWith(
      "https://agenthub.example.com/callback"
    );
    expect(setSlockCallbackInput).toHaveBeenCalledWith("next-code");
    expect(onSaveSlockLinker).toHaveBeenCalledTimes(1);
    expect(onCreateSlockLinkAttempt).toHaveBeenCalledTimes(1);
    expect(onExchangeSlockCode).toHaveBeenCalledTimes(1);
  });
});
