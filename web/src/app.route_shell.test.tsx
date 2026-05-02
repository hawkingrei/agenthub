// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const {
  authStatusMock,
  loginStartMock,
  listAgentsMock,
  listAgentEventsMock,
  listAgentNodesMock,
  listSafePathsMock,
  listDevicesMock,
  listAuditsMock,
  getVapidInfoMock,
  getRuntimeDefaultsMock,
  getAdminSettingsMock,
  agentsRouteShellPropsMock,
} = vi.hoisted(() => ({
  authStatusMock: vi.fn(),
  loginStartMock: vi.fn(),
  listAgentsMock: vi.fn(),
  listAgentEventsMock: vi.fn(),
  listAgentNodesMock: vi.fn(),
  listSafePathsMock: vi.fn(),
  listDevicesMock: vi.fn(),
  listAuditsMock: vi.fn(),
  getVapidInfoMock: vi.fn(),
  getRuntimeDefaultsMock: vi.fn(),
  getAdminSettingsMock: vi.fn(),
  agentsRouteShellPropsMock: vi.fn(),
}));

vi.mock("./api", () => ({
  api: {
    authStatus: authStatusMock,
    loginStart: loginStartMock,
    listAgents: listAgentsMock,
    listAgentEvents: listAgentEventsMock,
    listAgentNodes: listAgentNodesMock,
    listSafePaths: listSafePathsMock,
    listDevices: listDevicesMock,
    listAudits: listAuditsMock,
    getVapidInfo: getVapidInfoMock,
    getRuntimeDefaults: getRuntimeDefaultsMock,
    getAdminSettings: getAdminSettingsMock,
  },
  parseApiErrorMessage: vi.fn(() => null),
}));

vi.mock("./components/agents_route_shell", () => ({
  AgentsRouteShell: (props: unknown) => {
    agentsRouteShellPropsMock(props);
    return <div data-testid="agents-route-shell">Agents route shell</div>;
  },
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

import { App, RouteFallback } from "./app";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function renderApp(root: Root) {
  root.render(
    <MantineProvider>
      <App />
    </MantineProvider>
  );
}

async function flushRenderFrames(count: number): Promise<void> {
  for (let i = 0; i < count; i += 1) {
    await Promise.resolve();
    await new Promise((resolve) => window.setTimeout(resolve, 0));
  }
}

describe("App route shell wiring", () => {
  let container: HTMLDivElement;
  let root: Root;
  let originalLocalStorageDescriptor: PropertyDescriptor | undefined;
  let originalWindowMatchMediaDescriptor: PropertyDescriptor | undefined;
  let originalGlobalMatchMediaDescriptor: PropertyDescriptor | undefined;
  let originalWindowEventSourceDescriptor: PropertyDescriptor | undefined;
  let originalGlobalEventSourceDescriptor: PropertyDescriptor | undefined;

  beforeEach(() => {
    authStatusMock.mockReset();
    authStatusMock.mockResolvedValue({ root_initialized: true });
    loginStartMock.mockReset();
    listAgentsMock.mockReset();
    listAgentsMock.mockResolvedValue([]);
    listAgentEventsMock.mockReset();
    listAgentEventsMock.mockResolvedValue([]);
    listAgentNodesMock.mockReset();
    listAgentNodesMock.mockResolvedValue([]);
    listSafePathsMock.mockReset();
    listSafePathsMock.mockResolvedValue([]);
    listDevicesMock.mockReset();
    listDevicesMock.mockResolvedValue([]);
    listAuditsMock.mockReset();
    listAuditsMock.mockResolvedValue([]);
    getVapidInfoMock.mockReset();
    getVapidInfoMock.mockResolvedValue(null);
    getRuntimeDefaultsMock.mockReset();
    getRuntimeDefaultsMock.mockResolvedValue({
      default_worktree_root: "~/.agenthub/worktrees",
    });
    getAdminSettingsMock.mockReset();
    getAdminSettingsMock.mockResolvedValue({ passkey_enabled: false });
    agentsRouteShellPropsMock.mockReset();

    const storage = new Map<string, string>();
    originalLocalStorageDescriptor = Object.getOwnPropertyDescriptor(
      globalThis,
      "localStorage"
    );
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => {
          storage.set(key, value);
        },
        removeItem: (key: string) => {
          storage.delete(key);
        },
        clear: () => {
          storage.clear();
        },
      },
    });
    globalThis.localStorage.setItem(
      "agenthub_auth",
      JSON.stringify({
        token: "token-root",
        userId: "user-1",
        username: "root",
        role: "root",
      })
    );

    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
    window.history.replaceState({}, "", "/");

    originalWindowMatchMediaDescriptor = Object.getOwnPropertyDescriptor(
      window,
      "matchMedia"
    );
    originalGlobalMatchMediaDescriptor = Object.getOwnPropertyDescriptor(
      globalThis,
      "matchMedia"
    );
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: () => {},
        removeListener: () => {},
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => false,
      })),
    });
    Object.defineProperty(globalThis, "matchMedia", {
      configurable: true,
      writable: true,
      value: window.matchMedia,
    });
    originalWindowEventSourceDescriptor = Object.getOwnPropertyDescriptor(
      window,
      "EventSource"
    );
    originalGlobalEventSourceDescriptor = Object.getOwnPropertyDescriptor(
      globalThis,
      "EventSource"
    );
    class MockEventSource {
      close() {}
      addEventListener() {}
      removeEventListener() {}
      onopen: ((event: Event) => void) | null = null;
      onerror: ((event: Event) => void) | null = null;
      onmessage: ((event: MessageEvent<string>) => void) | null = null;
      readonly readyState = 0;
      readonly url: string;

      constructor(url: string) {
        this.url = url;
      }
    }
    Object.defineProperty(window, "EventSource", {
      configurable: true,
      writable: true,
      value: MockEventSource,
    });
    Object.defineProperty(globalThis, "EventSource", {
      configurable: true,
      writable: true,
      value: MockEventSource,
    });
  });

  it("renders the shared route loading fallback chrome", () => {
    const html = renderToStaticMarkup(
      <MantineProvider>
        <RouteFallback label="Loading teams..." />
      </MantineProvider>
    );

    expect(html).toContain("Loading teams...");
    expect(html).toContain("loading this workspace route");
    expect(html).toContain("data-workspace-panel-loading=\"true\"");
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();

    if (originalLocalStorageDescriptor) {
      Object.defineProperty(globalThis, "localStorage", originalLocalStorageDescriptor);
    }
    if (originalWindowMatchMediaDescriptor) {
      Object.defineProperty(window, "matchMedia", originalWindowMatchMediaDescriptor);
    }
    if (originalGlobalMatchMediaDescriptor) {
      Object.defineProperty(globalThis, "matchMedia", originalGlobalMatchMediaDescriptor);
    }
    if (originalWindowEventSourceDescriptor) {
      Object.defineProperty(window, "EventSource", originalWindowEventSourceDescriptor);
    }
    if (originalGlobalEventSourceDescriptor) {
      Object.defineProperty(globalThis, "EventSource", originalGlobalEventSourceDescriptor);
    }

    vi.restoreAllMocks();
  });

  it("passes null workbench props when no runnable agent is auto-selected", async () => {
    listAgentsMock.mockResolvedValue([
      {
        id: "agent-exited",
        name: "Exited agent",
        workdir: "/tmp/exited",
        command: "codex",
        args: [],
        worktree_mode: "use_existing",
        code_mode: false,
        status: "exited",
        created_at: 1,
        updated_at: 2,
      },
    ]);

    act(() => {
      renderApp(root);
    });
    await flushRenderFrames(2);

    const lastCall = agentsRouteShellPropsMock.mock.calls[
      agentsRouteShellPropsMock.mock.calls.length - 1
    ]?.[0] as
      | { workbenchProps: unknown }
      | undefined;

    expect(lastCall?.workbenchProps).toBeNull();
  });

  it("passes a populated workbench prop bag for the auto-selected running agent", async () => {
    listAgentsMock.mockResolvedValue([
      {
        id: "agent-running",
        name: "Running agent",
        workdir: "/tmp/running",
        command: "codex",
        args: [],
        worktree_mode: "use_existing",
        code_mode: false,
        status: "running",
        created_at: 1,
        updated_at: 2,
      },
    ]);

    act(() => {
      renderApp(root);
    });
    await flushRenderFrames(10);

    const populatedCall = agentsRouteShellPropsMock.mock.calls
      .map((call) => call[0] as {
        workbenchProps:
          | {
              activeAgent: string;
              activeAgentRecord: { workdir: string } | null;
            }
          | null;
      })
      .find((call) => call.workbenchProps?.activeAgent === "agent-running");

    expect(populatedCall?.workbenchProps?.activeAgent).toBe("agent-running");
    expect(populatedCall?.workbenchProps?.activeAgentRecord?.workdir).toBe(
      "/tmp/running"
    );
    expect(listAgentEventsMock).toHaveBeenCalledWith(
      "token-root",
      "agent-running",
      20,
      undefined
    );
  });

  it("renders a non-empty root workbench node when the nodes lens is unsupported", async () => {
    globalThis.localStorage.setItem(
      "agenthub_auth",
      JSON.stringify({
        token: "token-user",
        userId: "user-2",
        username: "member",
        role: "user",
      })
    );
    window.history.replaceState({}, "", "/workspace/nodes/node-east");

    act(() => {
      renderApp(root);
    });
    await flushRenderFrames(10);

    const lastCall = agentsRouteShellPropsMock.mock.calls[
      agentsRouteShellPropsMock.mock.calls.length - 1
    ]?.[0] as
      | {
          rootWorkbenchNode: React.ReactNode;
        }
      | undefined;

    expect(lastCall?.rootWorkbenchNode).toBeTruthy();
    const rootWorkbenchHtml = renderToStaticMarkup(
      <MantineProvider>{lastCall?.rootWorkbenchNode}</MantineProvider>
    );
    expect(rootWorkbenchHtml).toContain("Machines unavailable");
    expect(rootWorkbenchHtml).toContain("do not have permission to manage machines");
  });

  it("renders the shared workspace search placeholder in the root shell", async () => {
    globalThis.localStorage.setItem(
      "agenthub_auth",
      JSON.stringify({
        token: "token-root",
        userId: "user-1",
        username: "root",
        role: "root",
      })
    );
    window.history.replaceState({}, "", "/workspace?lens=search");

    act(() => {
      renderApp(root);
    });
    await flushRenderFrames(10);

    const lastCall = agentsRouteShellPropsMock.mock.calls[
      agentsRouteShellPropsMock.mock.calls.length - 1
    ]?.[0] as
      | {
          rootWorkbenchNode: React.ReactNode;
        }
      | undefined;

    expect(lastCall?.rootWorkbenchNode).toBeTruthy();
    const rootWorkbenchHtml = renderToStaticMarkup(
      <MantineProvider>{lastCall?.rootWorkbenchNode}</MantineProvider>
    );
    expect(rootWorkbenchHtml).toContain("Shared search is still being wired in");
    expect(rootWorkbenchHtml).toContain("shell-level placeholder");
    expect(rootWorkbenchHtml).toContain("data-workspace-lens-placeholder=\"search\"");
  });

  it("omits the agent output header when the nodes lens is active", async () => {
    globalThis.localStorage.setItem(
      "agenthub_auth",
      JSON.stringify({
        token: "token-root",
        userId: "user-1",
        username: "root",
        role: "root",
      })
    );
    window.history.replaceState({}, "", "/workspace/nodes/node-east");

    act(() => {
      renderApp(root);
    });
    await flushRenderFrames(10);

    const lastCall = agentsRouteShellPropsMock.mock.calls[
      agentsRouteShellPropsMock.mock.calls.length - 1
    ]?.[0] as
      | {
          showOutputHeader?: boolean;
        }
      | undefined;

    expect(lastCall?.showOutputHeader).toBe(false);
  });
});
