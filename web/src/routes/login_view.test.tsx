import { MantineProvider } from "@mantine/core";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { LoginView } from "./login_view";

describe("LoginView", () => {
  const baseProps = {
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

  function renderHtml(props: any = {}) {
    return renderToStaticMarkup(
      <MantineProvider>
        <LoginView {...baseProps} {...props} />
      </MantineProvider>
    );
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
    expect(html).toContain("Initialize Root");
    expect(html).toContain('value="User One"');
  });

  it("shows busy states", () => {
    const loginBusyHtml = renderHtml({ authBusy: "login" });
    expect(loginBusyHtml).toContain("Logging in...");

    const registerBusyHtml = renderHtml({ authBusy: "register", rootInitialized: false });
    expect(registerBusyHtml).toContain("Bootstrapping...");
  });
});
