// @vitest-environment jsdom
import { MantineProvider } from "@mantine/core";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LoginView, type LoginViewProps } from "./login_view";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("LoginView", () => {
  let container: HTMLDivElement;
  let root: Root;

  const baseProps: LoginViewProps = {
    authBusy: null,
    rootInitialized: true,
    username: "user1",
    password: "password1",
    displayName: "User One",
    setUsername: vi.fn(),
    setPassword: vi.fn(),
    setDisplayName: vi.fn(),
    onLogin: vi.fn(),
    onRegister: vi.fn(),
  };

  beforeEach(() => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
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
    vi.clearAllMocks();
  });

  function renderHtml(props: Partial<LoginViewProps> = {}) {
    return renderToStaticMarkup(
      <MantineProvider>
        <LoginView {...baseProps} {...props} />
      </MantineProvider>
    );
  }

  function renderDom(props: Partial<LoginViewProps> = {}) {
    act(() => {
      root.render(
        <MantineProvider>
          <LoginView {...baseProps} {...props} />
        </MantineProvider>
      );
    });
  }

  function updateInputValue(input: HTMLInputElement, value: string) {
    const valueSetter = Object.getOwnPropertyDescriptor(
      window.HTMLInputElement.prototype,
      "value"
    )?.set;
    valueSetter?.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }

  it("renders login form correctly", () => {
    const html = renderHtml();
    expect(html).toContain("Login");
    expect(html).toContain('value="user1"');
    expect(html).toContain('value="password1"');
    expect(html).not.toContain("Initialize Root");
  });

  it("renders register fields when root is not initialized", () => {
    const html = renderHtml({ rootInitialized: false });
    expect(html).toContain("First-run setup");
    expect(html).toContain("Root required");
    expect(html).toContain("Initialize Root");
    expect(html).toContain('value="User One"');
    expect(html).toContain("Root account bootstrap");
    expect(html).toContain("Server role and provider credentials remain operator-managed.");
  });

  it("keeps setup guidance out of the normal login state", () => {
    const html = renderHtml({ rootInitialized: true });
    expect(html).toContain("Login");
    expect(html).not.toContain("First-run setup");
    expect(html).not.toContain("Root account bootstrap");
  });

  it("renders a setup loading state while auth status is unknown", () => {
    const html = renderHtml({ rootInitialized: null });
    expect(html).toContain("Checking setup");
    expect(html).toContain('aria-busy="true"');
    expect(html).not.toContain("Login");
    expect(html).not.toContain("First-run setup");
  });

  it("shows busy states", () => {
    const loginBusyHtml = renderHtml({ authBusy: "login" });
    expect(loginBusyHtml).toContain("Logging in...");

    const registerBusyHtml = renderHtml({ authBusy: "register", rootInitialized: false });
    expect(registerBusyHtml).toContain("Bootstrapping...");
  });

  it("submits login from the normal login state", () => {
    const onLogin = vi.fn().mockResolvedValue(undefined);
    renderDom({ onLogin });

    const form = container.querySelector("form");
    expect(form).not.toBeNull();
    act(() => {
      form?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });

    expect(onLogin).toHaveBeenCalledTimes(1);
  });

  it("submits root initialization from the first-run state", () => {
    const onLogin = vi.fn().mockResolvedValue(undefined);
    const onRegister = vi.fn().mockResolvedValue(undefined);
    renderDom({ rootInitialized: false, onLogin, onRegister });

    const form = container.querySelector("form");
    expect(form).not.toBeNull();
    act(() => {
      form?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    });

    expect(onRegister).toHaveBeenCalledWith("root");
    expect(onLogin).not.toHaveBeenCalled();
  });

  it("initializes the root operator from the first-run state", () => {
    const onRegister = vi.fn().mockResolvedValue(undefined);
    renderDom({ rootInitialized: false, onRegister });

    const initializeButton = Array.from(container.querySelectorAll("button")).find(
      (button) => button.textContent === "Initialize Root"
    );
    expect(initializeButton).toBeDefined();
    act(() => {
      initializeButton?.click();
    });

    expect(onRegister).toHaveBeenCalledWith("root");
  });

  it("forwards first-run field edits to parent state", () => {
    const setUsername = vi.fn();
    const setPassword = vi.fn();
    const setDisplayName = vi.fn();
    renderDom({
      rootInitialized: false,
      setUsername,
      setPassword,
      setDisplayName,
    });

    act(() => {
      const usernameInput = container.querySelector<HTMLInputElement>("#login-username");
      const passwordInput = container.querySelector<HTMLInputElement>("#login-password");
      const displayNameInput =
        container.querySelector<HTMLInputElement>("#login-display-name");
      updateInputValue(usernameInput!, "new-user");
      updateInputValue(passwordInput!, "new-password");
      updateInputValue(displayNameInput!, "New User");
    });

    expect(setUsername).toHaveBeenCalledWith("new-user");
    expect(setPassword).toHaveBeenCalledWith("new-password");
    expect(setDisplayName).toHaveBeenCalledWith("New User");
  });
});
