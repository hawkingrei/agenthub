import React from "react";
import type {
  AgentNodeJoinBootstrapInfo,
  AgentNodeRecord,
  AgentRecord,
  TeamDefinitionRecord,
} from "../api";
import { canManageAgentNodes } from "../app_agents_helpers";
import { type WorkspaceLens } from "../app_route_selection";
import { AgentNodesWorkbench } from "../components/agent_nodes_workbench";
import { AgentsRootPage, type AgentsRootPageProps } from "../components/agents_root_page";
import {
  buildOutputHeaderProps,
} from "../components/agents_route_shell_props";
import type { AgentsWorkbenchProps } from "../components/agents_workbench_types";
import type { OutputHeaderProps } from "../components/output_header";
import {
  WorkspaceMachinesUnavailablePlaceholder,
  WorkspaceSearchLensPlaceholder,
  WorkspaceChannelsLensPlaceholder,
  WorkspaceTasksLensPlaceholder,
  WorkspaceMembersLensPlaceholder,
} from "../components/workspace_lens_placeholder";

type AgentsRouteContainerProps = Omit<
  AgentsRootPageProps,
  "outputHeaderProps" | "showOutputHeader" | "workbenchProps" | "rootWorkbenchNode"
> & {
  activeWorkspaceLens: WorkspaceLens;
  outputHeaderProps: OutputHeaderProps;
  workbenchProps: AgentsWorkbenchProps | null;
  nodes: AgentNodeRecord[];
  agents: AgentRecord[];
  teams: TeamDefinitionRecord[];
  teamMemberAgentsById: Record<string, AgentRecord | null>;
  selectedNodeId: string;
  nodeJoinBootstrap: AgentNodeJoinBootstrapInfo | null;
  nodeJoinBootstrapLoading: boolean;
  nodeJoinBootstrapError: string | null;
  updatingNodeIds: Record<string, boolean>;
  deletingNodeIds: Record<string, boolean>;
  onSelectNode: (nodeId: string) => void;
  onOpenNodeAgent: (agentId: string) => void;
  onCreateAgent: () => void;
  onUpdateNode: (nodeId: string, payload: { name: string; grpc_target: string; tls_server_name?: string | null; default_worktree_root?: string | null; }) => void;
  onDeleteNode: (nodeId: string) => void;
};

export function AgentsRouteContainer({
  activeWorkspaceLens,
  auth,
  outputHeaderProps,
  workbenchProps,
  nodes,
  agents,
  teams,
  teamMemberAgentsById,
  selectedNodeId,
  nodeJoinBootstrap,
  nodeJoinBootstrapLoading,
  nodeJoinBootstrapError,
  updatingNodeIds,
  deletingNodeIds,
  onSelectNode,
  onOpenNodeAgent,
  onCreateAgent,
  onUpdateNode,
  onDeleteNode,
  ...pageProps
}: AgentsRouteContainerProps) {
  const routeOutputHeaderProps = React.useMemo(
    () =>
      activeWorkspaceLens === "nodes"
        ? buildOutputHeaderProps({
            activeAgent: null,
            activeSessionId: null,
            developerMode: outputHeaderProps.developerMode,
            hasAcp: false,
            thinkingStartTs: null,
            runStatus: null,
            modelLabel: null,
          })
        : outputHeaderProps,
    [activeWorkspaceLens, outputHeaderProps]
  );

  const routeWorkbenchProps = activeWorkspaceLens === "nodes" ? null : workbenchProps;
  const showRouteOutputHeader = activeWorkspaceLens !== "nodes";
  const canManageNodes = canManageAgentNodes(auth);
  const rootWorkbenchNode = React.useMemo(
    () =>
      activeWorkspaceLens === "nodes"
        ? canManageNodes
          ? (
            <AgentNodesWorkbench
              nodes={nodes}
              agents={agents}
              teams={teams}
              teamMemberAgentsById={teamMemberAgentsById}
              selectedNodeId={selectedNodeId}
              nodeJoinBootstrap={nodeJoinBootstrap}
              nodeJoinBootstrapLoading={nodeJoinBootstrapLoading}
              nodeJoinBootstrapError={nodeJoinBootstrapError}
              updatingNodeIds={updatingNodeIds}
              deletingNodeIds={deletingNodeIds}
              onSelectNode={onSelectNode}
              onOpenAgent={onOpenNodeAgent}
              onCreateAgent={onCreateAgent}
              onUpdateNode={onUpdateNode}
              onDeleteNode={onDeleteNode}
            />
          )
          : (
            <WorkspaceMachinesUnavailablePlaceholder className="m-6 text-center" />
          )
        : activeWorkspaceLens === "search"
          ? (
            <WorkspaceSearchLensPlaceholder className="m-4" />
          )
          : activeWorkspaceLens === "channels"
            ? (
              <WorkspaceChannelsLensPlaceholder className="m-4" />
            )
            : activeWorkspaceLens === "tasks"
              ? (
                <WorkspaceTasksLensPlaceholder className="m-4" />
              )
              : activeWorkspaceLens === "members"
                ? (
                  <WorkspaceMembersLensPlaceholder className="m-4" />
                )
                : null,
    [
      activeWorkspaceLens,
      canManageNodes,
      nodes,
      agents,
      teams,
      teamMemberAgentsById,
      selectedNodeId,
      nodeJoinBootstrap,
      nodeJoinBootstrapLoading,
      nodeJoinBootstrapError,
      updatingNodeIds,
      deletingNodeIds,
      onSelectNode,
      onOpenNodeAgent,
      onCreateAgent,
      onUpdateNode,
      onDeleteNode,
    ]
  );

  return (
    <AgentsRootPage
      {...pageProps}
      auth={auth}
      outputHeaderProps={routeOutputHeaderProps}
      showOutputHeader={showRouteOutputHeader}
      workbenchProps={routeWorkbenchProps}
      rootWorkbenchNode={rootWorkbenchNode}
    />
  );
}
