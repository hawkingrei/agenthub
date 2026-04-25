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
  parseApiErrorMessageMock,
} = vi.hoisted(() => ({
  authStatusMock: vi.fn(),
  listSafePathsMock: vi.fn(),
  listDevicesMock: vi.fn(),
  listAuditsMock: vi.fn(),
  getVapidInfoMock: vi.fn(),
  getAdminSettingsMock: vi.fn(),
  joinStartAdminMock: vi.fn(),
  parseApiErrorMessageMock: vi.fn<(error: unknown) => string | null>(() => null),
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
    setPasskeyEnabled: vi.fn(),
    addSafePath: vi.fn(),
    deleteSafePath: vi.fn(),
    revokeDevice: vi.fn(),
    rotateVapid: vi.fn(),
  },
  parseApiErrorMessage: parseApiErrorMessageMock,
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
    parseApiErrorMessageMock.mockReset();

    authStatusMock.mockResolvedValue({
      root_initialized: true,
      passkey_enabled: false,
    });
    listSafePathsMock.mockResolvedValue([]);
    listDevicesMock.mockResolvedValue([]);
    listAuditsMock.mockResolvedValue([]);
    getVapidInfoMock.mockResolvedValue(null);
    getAdminSettingsMock.mockResolvedValue({ passkey_enabled: false });
    parseApiErrorMessageMock.mockReturnValue(null);
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
});
