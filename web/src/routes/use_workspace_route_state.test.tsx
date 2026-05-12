// @vitest-environment jsdom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RouteLocationState } from "../app_route_selection";
import type { AuthState } from "../types";
import { useWorkspaceRouteState } from "./use_workspace_route_state";

const rootAuth: AuthState = {
  token: "token-root",
  userId: "user-root",
  username: "root",
  role: "root",
};

const userAuth: AuthState = {
  token: "token-user",
  userId: "user-normal",
  username: "alice",
  role: "user",
};

type WorkspaceRouteSnapshot = ReturnType<typeof useWorkspaceRouteState>;

function WorkspaceRouteProbe({
  activeAgent = "agent-1",
  auth = rootAuth,
  navigateWorkbenchRoute,
  onSnapshot,
  routeLocation,
}: {
  activeAgent?: string;
  auth?: AuthState | null;
  navigateWorkbenchRoute: (pathname: string) => void;
  onSnapshot: (snapshot: WorkspaceRouteSnapshot) => void;
  routeLocation: RouteLocationState;
}) {
  const snapshot = useWorkspaceRouteState({
    activeAgent,
    auth,
    navigateWorkbenchRoute,
    routeLocation,
  });
  onSnapshot(snapshot);
  return null;
}

describe("useWorkspaceRouteState", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
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

  function renderProbe(
    routeLocation: RouteLocationState,
    overrides: Partial<{
      activeAgent: string;
      auth: AuthState | null;
      navigateWorkbenchRoute: (pathname: string) => void;
    }> = {},
  ) {
    const snapshots: WorkspaceRouteSnapshot[] = [];
    const navigateWorkbenchRoute = overrides.navigateWorkbenchRoute ?? vi.fn();
    act(() => {
      root.render(
        <WorkspaceRouteProbe
          activeAgent={overrides.activeAgent}
          auth={overrides.auth}
          navigateWorkbenchRoute={navigateWorkbenchRoute}
          routeLocation={routeLocation}
          onSnapshot={(snapshot) => {
            snapshots.push(snapshot);
          }}
        />,
      );
    });
    const latest = snapshots[snapshots.length - 1];
    if (!latest) {
      throw new Error("workspace route snapshot was not captured");
    }
    return { navigateWorkbenchRoute, snapshot: latest };
  }

  it("derives canonical node route state and root-only lens items", () => {
    const { snapshot } = renderProbe({
      pathname: "/workspace/nodes/node-east",
      search: "",
    });

    expect(snapshot.activeWorkspaceLens).toBe("nodes");
    expect(snapshot.routeAgentId).toBeNull();
    expect(snapshot.selectedWorkspaceNodeId).toBe("node-east");
    expect(snapshot.effectiveSelectedWorkspaceNodeId).toBe("node-east");
    expect(snapshot.workspaceLensItems.map((item) => item.value)).toContain("nodes");

    const userSnapshot = renderProbe(
      {
        pathname: "/workspace/nodes/node-east",
        search: "",
      },
      { auth: userAuth },
    ).snapshot;
    expect(userSnapshot.workspaceLensItems.map((item) => item.value)).not.toContain("nodes");
  });

  it("preserves legacy node route selection while emitting canonical navigation targets", () => {
    const navigateWorkbenchRoute = vi.fn();
    const { snapshot } = renderProbe(
      {
        pathname: "/workspace",
        search: "?lens=nodes&node=node-east",
      },
      { navigateWorkbenchRoute },
    );

    expect(snapshot.activeWorkspaceLens).toBe("nodes");
    expect(snapshot.selectedWorkspaceNodeId).toBe("node-east");

    act(() => {
      snapshot.onSelectWorkspaceLens("nodes");
    });
    expect(navigateWorkbenchRoute).toHaveBeenLastCalledWith("/workspace/nodes/node-east");

    act(() => {
      snapshot.onSelectWorkspaceLens("teams");
    });
    expect(navigateWorkbenchRoute).toHaveBeenLastCalledWith("/workspace/teams");

    act(() => {
      snapshot.onSelectWorkspaceLens("search");
    });
    expect(navigateWorkbenchRoute).toHaveBeenLastCalledWith(
      "/workspace/agents/agent-1?lens=search",
    );
  });

  it("keeps the returned route-state object stable across equivalent renders", () => {
    const navigateWorkbenchRoute = vi.fn();
    const routeLocation = {
      pathname: "/workspace/nodes/node-east",
      search: "",
    };
    const snapshots: WorkspaceRouteSnapshot[] = [];

    act(() => {
      root.render(
        <WorkspaceRouteProbe
          navigateWorkbenchRoute={navigateWorkbenchRoute}
          routeLocation={routeLocation}
          onSnapshot={(snapshot) => {
            snapshots.push(snapshot);
          }}
        />,
      );
    });
    act(() => {
      root.render(
        <WorkspaceRouteProbe
          navigateWorkbenchRoute={navigateWorkbenchRoute}
          routeLocation={routeLocation}
          onSnapshot={(snapshot) => {
            snapshots.push(snapshot);
          }}
        />,
      );
    });

    expect(snapshots).toHaveLength(2);
    expect(snapshots[1]).toBe(snapshots[0]);
  });
});
