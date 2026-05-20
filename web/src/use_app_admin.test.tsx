// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppAdmin } from "./use_app_admin";

const {
  authStatusMock,
  listSafePathsMock,
  listDevicesMock,
  listAuditsMock,
  getVapidInfoMock,
  getAdminSettingsMock,
  joinStartAdminMock,
  setPasskeyEnabledMock,
  addSafePathMock,
  deleteSafePathMock,
  revokeDeviceMock,
  rotateVapidMock,
  listLinkersMock,
  upsertSlockLinkerMock,
  createSlockLinkAttemptMock,
  exchangeSlockCodeMock,
  parseApiErrorMessageMock,
  stringifyApiErrorMock,
} = vi.hoisted(() => ({
  authStatusMock: vi.fn(),
  listSafePathsMock: vi.fn(),
  listDevicesMock: vi.fn(),
  listAuditsMock: vi.fn(),
  getVapidInfoMock: vi.fn(),
  getAdminSettingsMock: vi.fn(),
  joinStartAdminMock: vi.fn(),
  setPasskeyEnabledMock: vi.fn(),
  addSafePathMock: vi.fn(),
  deleteSafePathMock: vi.fn(),
  revokeDeviceMock: vi.fn(),
  rotateVapidMock: vi.fn(),
  listLinkersMock: vi.fn(),
  upsertSlockLinkerMock: vi.fn(),
  createSlockLinkAttemptMock: vi.fn(),
  exchangeSlockCodeMock: vi.fn(),
  parseApiErrorMessageMock: vi.fn<(error: unknown) => string | null>(() => null),
  stringifyApiErrorMock: vi.fn<(error: unknown) => string>(() => "error"),
}));

vi.mock("./api", () => ({
  api: {
    authStatus: authStatusMock,
    listSafePaths: listSafePathsMock,
    listDevices: listDevicesMock,
    listAudits: listAuditsMock,
    getVapidInfo: getVapidInfoMock,
    getAdminSettings: getAdminSettingsMock,
    joinStartAdmin: joinStartAdminMock,
    setPasskeyEnabled: setPasskeyEnabledMock,
    addSafePath: addSafePathMock,
    deleteSafePath: deleteSafePathMock,
    revokeDevice: revokeDeviceMock,
    rotateVapid: rotateVapidMock,
    listLinkers: listLinkersMock,
    upsertSlockLinker: upsertSlockLinkerMock,
    createSlockLinkAttempt: createSlockLinkAttemptMock,
    exchangeSlockCode: exchangeSlockCodeMock,
  },
  parseApiErrorMessage: parseApiErrorMessageMock,
  stringifyApiError: stringifyApiErrorMock,
}));

type UseAppAdminResult = ReturnType<typeof useAppAdmin>;
type HookProps = Parameters<typeof useAppAdmin>;

function HookHarness({
  auth,
  isAdminRoute,
  onCapture,
}: {
  auth: HookProps[0];
  isAdminRoute: HookProps[1];
  onCapture: (value: UseAppAdminResult) => void;
}) {
  const value = useAppAdmin(auth, isAdminRoute);
  useEffect(() => {
    onCapture(value);
  }, [onCapture, value]);
  return null;
}

describe("useAppAdmin", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);

    authStatusMock.mockReset();
    listSafePathsMock.mockReset();
    listDevicesMock.mockReset();
    listAuditsMock.mockReset();
    getVapidInfoMock.mockReset();
    getAdminSettingsMock.mockReset();
    joinStartAdminMock.mockReset();
    setPasskeyEnabledMock.mockReset();
    addSafePathMock.mockReset();
    deleteSafePathMock.mockReset();
    revokeDeviceMock.mockReset();
    rotateVapidMock.mockReset();
    listLinkersMock.mockReset();
    upsertSlockLinkerMock.mockReset();
    createSlockLinkAttemptMock.mockReset();
    exchangeSlockCodeMock.mockReset();
    parseApiErrorMessageMock.mockReset();
    stringifyApiErrorMock.mockReset();

    authStatusMock.mockResolvedValue({
      root_initialized: true,
      passkey_enabled: false,
    });
    listSafePathsMock.mockResolvedValue([]);
    listDevicesMock.mockResolvedValue([]);
    listAuditsMock.mockResolvedValue([]);
    getVapidInfoMock.mockResolvedValue(null);
    getAdminSettingsMock.mockResolvedValue({ passkey_enabled: false });
    setPasskeyEnabledMock.mockResolvedValue({ status: "ok" });
    addSafePathMock.mockResolvedValue({ status: "ok" });
    deleteSafePathMock.mockResolvedValue({ status: "ok" });
    revokeDeviceMock.mockResolvedValue({ status: "ok" });
    rotateVapidMock.mockResolvedValue({ public_key: "next-key" });
    listLinkersMock.mockResolvedValue([]);
    parseApiErrorMessageMock.mockReturnValue(null);
    stringifyApiErrorMock.mockImplementation(
      (error) => parseApiErrorMessageMock(error) ?? String(error)
    );
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllMocks();
  });

  it("encodes join tokens into the browser join url", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    joinStartAdminMock.mockResolvedValue({
      pin: "123456",
      token: "abc +/?",
    });

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onCreateJoin();
      await Promise.resolve();
    });

    const updated = captures[captures.length - 1];
    expect(joinStartAdminMock).toHaveBeenCalledWith("token-1");
    expect(updated.joinPin).toBe("123456");
    expect(updated.joinToken).toBe("abc +/?");
    expect(updated.joinUrl).toBe(
      `${location.origin}/join?token=${encodeURIComponent("abc +/?")}`
    );
  });

  it("clears stale join data when join creation fails", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    const error = new Error("join failed");
    joinStartAdminMock
      .mockResolvedValueOnce({
        pin: "123456",
        token: "stale-token",
      })
      .mockRejectedValueOnce(error);
    parseApiErrorMessageMock.mockReturnValue("unable to create join token");
    stringifyApiErrorMock.mockReturnValue("unable to create join token");

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onCreateJoin();
      await Promise.resolve();
    });

    let updated = captures[captures.length - 1];
    expect(updated.joinPin).toBe("123456");
    expect(updated.joinToken).toBe("stale-token");
    expect(updated.joinUrl).toBe(
      `${location.origin}/join?token=${encodeURIComponent("stale-token")}`
    );

    await act(async () => {
      await updated.onCreateJoin();
      await Promise.resolve();
    });

    updated = captures[captures.length - 1];
    expect(updated.error).toBe("unable to create join token");
    expect(updated.joinUrl).toBeNull();
    expect(updated.joinToken).toBeNull();
    expect(updated.joinPin).toBeNull();
  });

  it("loads and updates Slock linker state on the admin route", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    const configuredLinker = {
      linker_id: "slock-primary",
      connector_id: "slock",
      display_name: "Slock",
      status: "configured",
      api_origin: "https://api.slock.ai",
      client_id: "agenthub",
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
      scopes: ["identity", "openid", "profile"],
      client_secret_configured: true,
      token_configured: false,
      token_type: null,
      granted_scopes: [],
      expires_at: null,
      principal: null,
      updated_at: 1,
    };
    const connectedLinker = {
      ...configuredLinker,
      status: "connected",
      token_configured: true,
      token_type: "Bearer",
      granted_scopes: ["identity", "openid", "profile"],
      principal: {
        subject: "slock-agent-1",
        principal_type: "agent",
        display_name: "Claude Assistant",
        handle: "assistant",
        avatar_url: null,
        server_id: "server-1",
        server_slug: "dev",
        updated_at: 2,
      },
    };
    listLinkersMock.mockResolvedValue([configuredLinker]);
    upsertSlockLinkerMock.mockResolvedValue(configuredLinker);
    createSlockLinkAttemptMock.mockResolvedValue({
      linker_id: "slock-primary",
      state: "state-1",
      expires_at: 100,
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
    });
    exchangeSlockCodeMock.mockResolvedValue(connectedLinker);

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    expect(listLinkersMock).toHaveBeenCalledWith("token-1");
    expect(latest.slockLinker?.linker_id).toBe("slock-primary");
    expect(latest.slockClientId).toBe("agenthub");

    await act(async () => {
      latest.setSlockClientSecret("secret-1");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onSaveSlockLinker();
      await Promise.resolve();
    });
    expect(upsertSlockLinkerMock).toHaveBeenCalledWith("token-1", {
      api_origin: "https://api.slock.ai",
      client_id: "agenthub",
      client_secret: "secret-1",
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
      scopes: ["identity", "openid", "profile"],
    });

    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onCreateSlockLinkAttempt();
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    expect(latest.slockLinkAttempt?.state).toBe("state-1");

    await act(async () => {
      latest.setSlockCallbackInput("callback-code");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onExchangeSlockCode();
      await Promise.resolve();
    });

    expect(exchangeSlockCodeMock).toHaveBeenCalledWith("token-1", {
      code: "callback-code",
      state: "state-1",
    });
    latest = captures[captures.length - 1];
    expect(latest.slockLinker?.principal?.display_name).toBe("Claude Assistant");
    expect(latest.slockCallbackInput).toBe("");
    expect(latest.slockLinkAttempt).toBeNull();
  });

  it("refreshes admin datasets after maintenance actions", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    listSafePathsMock.mockResolvedValue([{ path: "/workspace", created_at: 1 }]);
    listDevicesMock.mockResolvedValue([
      {
        id: "device-1",
        user_id: "user-1",
        name: "Laptop",
        user_agent: "browser",
        status: "active",
        created_at: 1,
        last_login_at: null,
      },
    ]);
    listAuditsMock.mockResolvedValue([
      {
        id: 1,
        user_id: "user-1",
        device_id: "device-1",
        event: "login",
        ip: null,
        user_agent: null,
        detail: null,
        ts: 1,
      },
    ]);
    getVapidInfoMock.mockResolvedValue({
      public_key: "vapid-key",
      subject: "mailto:admin@example.com",
      keys_path: "/tmp/vapid.json",
    });

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onPasskeyEnabledChange(true);
      latest.setSafePathInput("/workspace");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onAddSafePath();
      await latest.onDeleteSafePath("/workspace");
      await latest.onRevokeDevice("device-1");
      await latest.onRotateVapid();
      latest.onToggleSafePath("/workspace");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      latest.onToggleAllSafePaths();
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onDeleteSelectedSafePaths();
      await Promise.resolve();
    });

    expect(setPasskeyEnabledMock).toHaveBeenCalledWith("token-1", true);
    expect(addSafePathMock).toHaveBeenCalledWith("token-1", "/workspace");
    expect(deleteSafePathMock).toHaveBeenCalledWith("token-1", "/workspace");
    expect(revokeDeviceMock).toHaveBeenCalledWith("token-1", "device-1");
    expect(rotateVapidMock).toHaveBeenCalledWith("token-1");
    expect(latest.selectedSafePaths.size).toBe(0);
  });

  it("clears Slock linker state when leaving the root admin route", async () => {
    const captures: UseAppAdminResult[] = [];
    const configuredLinker = {
      linker_id: "slock-primary",
      connector_id: "slock",
      display_name: "Slock",
      status: "configured",
      api_origin: "https://api.slock.ai",
      client_id: "agenthub",
      return_url: "https://agenthub.example.com/api/linkers/slock/callback",
      scopes: ["identity", "openid", "profile"],
      client_secret_configured: true,
      token_configured: false,
      token_type: null,
      granted_scopes: [],
      expires_at: null,
      principal: null,
      updated_at: 1,
    };
    listLinkersMock.mockResolvedValue([configuredLinker]);

    await act(async () => {
      root.render(
        <HookHarness
          auth={{ token: "token-1", role: "root" } as HookProps[0]}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(captures[captures.length - 1].slockLinker?.linker_id).toBe(
      "slock-primary"
    );

    await act(async () => {
      root.render(
        <HookHarness
          auth={{ token: "token-1", role: "root" } as HookProps[0]}
          isAdminRoute={false}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    expect(latest.slockLinker).toBeNull();
    expect(latest.slockLinkAttempt).toBeNull();
    expect(listLinkersMock).toHaveBeenCalledTimes(1);
  });

  it("keeps admin state usable when initial linker settings fail to load", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    getAdminSettingsMock.mockRejectedValue(new Error("settings failed"));
    listLinkersMock.mockRejectedValue(new Error("linkers failed"));

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const latest = captures[captures.length - 1];
    expect(getAdminSettingsMock).toHaveBeenCalledWith("token-1");
    expect(listLinkersMock).toHaveBeenCalledWith("token-1");
    expect(latest.slockLinker).toBeNull();
    expect(latest.slockLinkAttempt).toBeNull();
    expect(latest.slockApiOrigin).toBe("https://api.slock.ai");
  });

  it("surfaces Slock action errors and exchanges callback URLs", async () => {
    const captures: UseAppAdminResult[] = [];
    const auth = { token: "token-1", role: "root" } as HookProps[0];
    const connectedLinker = {
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
      expires_at: null,
      principal: null,
      updated_at: 1,
    };
    stringifyApiErrorMock.mockReturnValue("Slock action failed");
    upsertSlockLinkerMock.mockRejectedValue(new Error("save failed"));
    createSlockLinkAttemptMock.mockRejectedValue(new Error("attempt failed"));
    exchangeSlockCodeMock
      .mockRejectedValueOnce(new Error("exchange failed"))
      .mockResolvedValueOnce(connectedLinker);

    await act(async () => {
      root.render(
        <HookHarness
          auth={auth}
          isAdminRoute={true}
          onCapture={(value) => captures.push(value)}
        />
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    let latest = captures[captures.length - 1];
    await act(async () => {
      latest.setSlockScopesInput(" identity   openid  profile ");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onSaveSlockLinker();
      await Promise.resolve();
    });
    expect(upsertSlockLinkerMock).toHaveBeenCalledWith("token-1", {
      api_origin: "https://api.slock.ai",
      client_id: "",
      client_secret: null,
      return_url: `${location.origin}/api/linkers/slock/callback`,
      scopes: ["identity", "openid", "profile"],
    });
    latest = captures[captures.length - 1];
    expect(latest.error).toBe("Slock action failed");

    await act(async () => {
      await latest.onCreateSlockLinkAttempt();
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    expect(latest.error).toBe("Slock action failed");

    await act(async () => {
      latest.setSlockCallbackInput("callback-code");
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onExchangeSlockCode();
      await Promise.resolve();
    });
    expect(exchangeSlockCodeMock).toHaveBeenCalledWith("token-1", {
      code: "callback-code",
      state: null,
    });
    latest = captures[captures.length - 1];
    expect(latest.error).toBe("Slock action failed");

    const callbackUrl =
      "https://agenthub.example.com/api/linkers/slock/callback?code=callback-code";
    await act(async () => {
      latest.setSlockCallbackInput(callbackUrl);
      await Promise.resolve();
    });
    latest = captures[captures.length - 1];
    await act(async () => {
      await latest.onExchangeSlockCode();
      await Promise.resolve();
    });

    expect(exchangeSlockCodeMock).toHaveBeenCalledWith("token-1", {
      callback_url: callbackUrl,
    });
    latest = captures[captures.length - 1];
    expect(latest.slockLinker?.status).toBe("connected");
    expect(latest.slockCallbackInput).toBe("");
  });
});
