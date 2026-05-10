import React, { Suspense } from "react";
import type { ConnectionBadge } from "../connection_status";
import { AuthState } from "../types";
import { AgentNodeSectionProps } from "./agent_node_section";
import { AgentsPanelProps } from "./agents_panel";
import { AgentsRouteShell } from "./agents_route_shell";
import { AgentsWorkbenchProps } from "./agents_workbench_types";
import { CreateAgentModalProps } from "./create_agent_modal";
import { OutputHeaderProps } from "./output_header";
import { PermissionModalProps } from "./permission_modal";
import { type WorkspaceShellLensItem } from "./workspace_shell_header";

import { WorkspaceShell } from "./layout/workspace_shell";
import { LoginView, type LoginViewProps } from "../routes/login_view";

const LazyCreateAgentModal = React.lazy(async () => {
  const module = await import("./create_agent_modal");
  return { default: module.CreateAgentModal };
});

const LazyAgentNodeSection = React.lazy(async () => {
  const module = await import("./agent_node_section");
  return { default: module.AgentNodeSection };
});

const LazyPermissionModal = React.lazy(async () => {
  const module = await import("./permission_modal");
  return { default: module.PermissionModal };
});

export type AgentsRootPageProps = LoginViewProps & {
  appRootRef: React.RefObject<HTMLDivElement | null>;
  appHeaderRef: React.RefObject<HTMLElement | null>;
  auth: AuthState | null;
  normalizedError: string | null;
  onClearError: () => void;
  agentsCollapsed: boolean;
  onCollapseAgents: () => void;
  onExpandAgents: () => void;
  connectionBadge: ConnectionBadge;
  onLogout: () => void;
  navigateWorkbenchRoute: (pathname: string) => void;
  workspaceRef: React.RefObject<HTMLElement | null>;
  workspaceStyle?: React.CSSProperties;
  onAgentsSplitterPointerDown: React.PointerEventHandler<HTMLDivElement>;
  agentsPanelProps: AgentsPanelProps;
  outputHeaderProps: OutputHeaderProps;
  showOutputHeader?: boolean;
  workbenchProps: AgentsWorkbenchProps | null;
  rootWorkbenchNode?: React.ReactNode;
  showCreateAgent: boolean;
  createAgentModalProps: CreateAgentModalProps;
  agentNodeSectionProps: AgentNodeSectionProps | null;
  permissionModalProps: PermissionModalProps | null;
  lensItems?: readonly WorkspaceShellLensItem[];
  onSelectLens?: (value: string) => void;
};

export const AgentsRootPage = React.memo(function AgentsRootPage({
  appRootRef,
  appHeaderRef,
  auth,
  normalizedError,
  onClearError,
  authBusy,
  rootInitialized,
  username,
  password,
  displayName,
  setUsername,
  setPassword,
  setDisplayName,
  onLogin,
  onRegister,
  agentsCollapsed,
  onCollapseAgents,
  onExpandAgents,
  connectionBadge,
  onLogout,
  navigateWorkbenchRoute,
  workspaceRef,
  workspaceStyle,
  onAgentsSplitterPointerDown,
  agentsPanelProps,
  outputHeaderProps,
  showOutputHeader = true,
  workbenchProps,
  rootWorkbenchNode = null,
  showCreateAgent,
  createAgentModalProps,
  agentNodeSectionProps,
  permissionModalProps,
  lensItems = [],
  onSelectLens,
}: AgentsRootPageProps) {
  const modals = (
    <>
      {auth && showCreateAgent ? (
        <Suspense fallback={null}>
          <LazyCreateAgentModal {...createAgentModalProps}>
            {agentNodeSectionProps ? (
              <Suspense fallback={null}>
                <LazyAgentNodeSection {...agentNodeSectionProps} />
              </Suspense>
            ) : null}
          </LazyCreateAgentModal>
        </Suspense>
      ) : null}

      {auth && permissionModalProps ? (
        <Suspense fallback={null}>
          <LazyPermissionModal {...permissionModalProps} />
        </Suspense>
      ) : null}
    </>
  );

  return (
    <WorkspaceShell
      appRootRef={appRootRef}
      headerRef={appHeaderRef}
      activeSurface="workspace"
      title="Workspace"
      subtitle={null}
      username={auth?.username ?? ""}
      isRoot={auth?.role === "root"}
      connectionBadge={connectionBadge}
      agentsCollapsed={agentsCollapsed}
      onToggleAgents={agentsCollapsed ? onExpandAgents : onCollapseAgents}
      normalizedError={normalizedError}
      onClearError={onClearError}
      lensItems={lensItems}
      onSelectLens={onSelectLens}
      onNavigate={navigateWorkbenchRoute}
      onLogout={onLogout}
      modals={modals}
    >
      {!auth ? (
        <LoginView
          authBusy={authBusy}
          rootInitialized={rootInitialized}
          username={username}
          password={password}
          displayName={displayName}
          setUsername={setUsername}
          setPassword={setPassword}
          setDisplayName={setDisplayName}
          onLogin={onLogin}
          onRegister={onRegister}
        />
      ) : (
        <AgentsRouteShell
          agentsCollapsed={agentsCollapsed}
          workspaceRef={workspaceRef}
          authToken={auth?.token ?? null}
          workspaceStyle={workspaceStyle}
          onAgentsSplitterPointerDown={onAgentsSplitterPointerDown}
          agentsPanelProps={agentsPanelProps}
          outputHeaderProps={outputHeaderProps}
          showOutputHeader={showOutputHeader}
          workbenchProps={workbenchProps}
          rootWorkbenchNode={rootWorkbenchNode}
        />
      )}
    </WorkspaceShell>
  );
});
AgentsRootPage.displayName = "AgentsRootPage";
