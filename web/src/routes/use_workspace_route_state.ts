import { useCallback, useMemo } from "react";
import { canManageAgentNodes } from "../app_agents_helpers";
import {
  buildWorkspaceNodePath,
  buildWorkspacePath,
  resolveWorkspaceAgentRoute,
  resolveWorkspaceLens,
  resolveWorkspaceNodeId,
  resolveWorkspaceNodeRoute,
  type RouteLocationState,
  type WorkspaceLens,
} from "../app_route_selection";
import { buildStandardWorkspaceLensItems } from "../components/workspace_lens_items";
import { buildTeamSelectorPath } from "../pages/team/team_route_helpers";
import type { AuthState } from "../types";

type UseWorkspaceRouteStateArgs = {
  activeAgent: string | null;
  auth: AuthState | null;
  navigateWorkbenchRoute: (pathname: string) => void;
  routeLocation: RouteLocationState;
};

export function useWorkspaceRouteState({
  activeAgent,
  auth,
  navigateWorkbenchRoute,
  routeLocation,
}: UseWorkspaceRouteStateArgs) {
  const workspaceAgentRoute = useMemo(
    () => resolveWorkspaceAgentRoute(routeLocation.pathname),
    [routeLocation.pathname],
  );
  const workspaceNodeRoute = useMemo(
    () => resolveWorkspaceNodeRoute(routeLocation.pathname, routeLocation.search),
    [routeLocation.pathname, routeLocation.search],
  );
  const routeAgentId =
    workspaceAgentRoute?.mode === "agent" ? workspaceAgentRoute.agentId : null;
  const activeWorkspaceLens = useMemo(
    () =>
      workspaceNodeRoute
        ? "nodes"
        : resolveWorkspaceLens(routeLocation.pathname, routeLocation.search),
    [routeLocation.pathname, routeLocation.search, workspaceNodeRoute],
  );
  const selectedWorkspaceNodeId = useMemo(
    () =>
      workspaceNodeRoute?.nodeId ?? resolveWorkspaceNodeId(routeLocation.search),
    [routeLocation.search, workspaceNodeRoute],
  );
  const effectiveSelectedWorkspaceNodeId = selectedWorkspaceNodeId ?? "main";
  const workspaceLensItems = useMemo(
    () =>
      buildStandardWorkspaceLensItems(activeWorkspaceLens, {
        includeNodes: canManageAgentNodes(auth),
      }),
    [activeWorkspaceLens, auth],
  );

  const onSelectWorkspaceLens = useCallback(
    (value: string) => {
      const lens = value as WorkspaceLens;
      if (lens === "teams") {
        navigateWorkbenchRoute(buildTeamSelectorPath());
        return;
      }
      if (lens === "nodes") {
        navigateWorkbenchRoute(buildWorkspaceNodePath(selectedWorkspaceNodeId));
        return;
      }
      navigateWorkbenchRoute(buildWorkspacePath(activeAgent, lens));
    },
    [activeAgent, navigateWorkbenchRoute, selectedWorkspaceNodeId],
  );

  return useMemo(
    () => ({
      activeWorkspaceLens,
      effectiveSelectedWorkspaceNodeId,
      onSelectWorkspaceLens,
      routeAgentId,
      selectedWorkspaceNodeId,
      workspaceAgentRoute,
      workspaceLensItems,
      workspaceNodeRoute,
    }),
    [
      activeWorkspaceLens,
      effectiveSelectedWorkspaceNodeId,
      onSelectWorkspaceLens,
      routeAgentId,
      selectedWorkspaceNodeId,
      workspaceAgentRoute,
      workspaceLensItems,
      workspaceNodeRoute,
    ],
  );
}
