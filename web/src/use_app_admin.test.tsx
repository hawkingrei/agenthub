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
    listLinkers: listLinkersMock,
    upsertSlockLinker: upsertSlockLinkerMock,
    createSlockLinkAttempt: createSlockLinkAttemptMock,
    exchangeSlockCode: exchangeSlockCodeMock,
    setPasskeyEnabled: vi.fn(),
    addSafePath: vi.fn(),
    deleteSafePath: vi.fn(),
    revokeDevice: vi.fn(),
    rotateVapid: vi.fn(),
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
});
