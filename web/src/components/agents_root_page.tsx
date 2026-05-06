import React, { Suspense } from "react";
import type { ConnectionBadge } from "../connection_status";
import { ErrorBanner } from "../error_banner";
import { AuthState } from "../types";
import { ActionButton } from "../ui/primitives";
import {
  APP_WORKBENCH_ACCOUNT_MENU_BUTTON_CLASS,
  APP_WORKBENCH_HEADER_CLASS,
  APP_WORKBENCH_HEADER_STATUS_CLASS,
  APP_WORKBENCH_SIDEBAR_TOGGLE_BUTTON_CLASS,
  AUTH_ACTIONS_CLASS,
  AUTH_FORM_CARD_CLASS,
  AUTH_INPUT_CLASS,
  AUTH_PRIMARY_BUTTON_CLASS,
  AUTH_SECONDARY_BUTTON_CLASS,
} from "../ui/tailwind_classes";
import { AgentNodeSectionProps } from "./agent_node_section";
import { AgentsPanelProps } from "./agents_panel";
import { AgentsRouteShell } from "./agents_route_shell";
import { AgentsWorkbenchProps } from "./agents_workbench_types";
import { CreateAgentModalProps } from "./create_agent_modal";
import { OutputHeaderProps } from "./output_header";
import { PermissionModalProps } from "./permission_modal";
import { WorkspaceShellHeader, type WorkspaceShellLensItem } from "./workspace_shell_header";

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

export type AgentsRootPageProps = {
  appRootRef: React.RefObject<HTMLDivElement | null>;
  appHeaderRef: React.RefObject<HTMLElement | null>;
  auth: AuthState | null;
  normalizedError: string | null;
  onClearError: () => void;
  authBusy: "login" | "register" | null;
  rootInitialized: boolean | null;
  username: string;
  password: string;
  displayName: string;
  setUsername: (value: string) => void;
  setPassword: (value: string) => void;
  setDisplayName: (value: string) => void;
  onLogin: () => Promise<void>;
  onRegister: (role: string) => Promise<void>;
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
  return (
    <div className="flex h-screen flex-col overflow-hidden bg-white" ref={appRootRef as React.Ref<HTMLDivElement>}>
      {auth ? (
        <WorkspaceShellHeader
          ref={appHeaderRef as React.Ref<HTMLElement>}
          activeSurface="workspace"
          title="Workspace"
          subtitle={null}
          sidebarToggleLabel={agentsCollapsed ? "Show agents" : "Hide agents"}
          sidebarCollapsed={agentsCollapsed}
          onToggleSidebar={agentsCollapsed ? onExpandAgents : onCollapseAgents}
          username={auth.username}
          isRoot={auth.role === "root"}
          headerShellClassName={APP_WORKBENCH_HEADER_CLASS}
          headerIconButtonClassName={`${APP_WORKBENCH_SIDEBAR_TOGGLE_BUTTON_CLASS} ${
            agentsCollapsed ? "bg-white" : "bg-notion-hover text-notion-text"
          }`}
          menuButtonClassName={APP_WORKBENCH_ACCOUNT_MENU_BUTTON_CLASS}
          connectionBadge={connectionBadge}
          headerStatusClassName={APP_WORKBENCH_HEADER_STATUS_CLASS}
          lensItems={lensItems}
          onSelectLens={onSelectLens}
          onNavigate={navigateWorkbenchRoute}
          onLogout={onLogout}
        />
      ) : null}

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {normalizedError ? (
          <div className="px-4 py-2 sm:px-6">
            <ErrorBanner message={normalizedError} onClose={onClearError} />
          </div>
        ) : null}

      {!auth ? (
        <form
          className={AUTH_FORM_CARD_CLASS}
          onSubmit={(event) => {
            event.preventDefault();
            void onLogin();
          }}
        >
          <h2 className="text-xl font-bold tracking-tight text-notion-text">
            Login
          </h2>
          <input
            className={AUTH_INPUT_CLASS}
            id="login-username"
            name="username"
            placeholder="Username"
            value={username}
            disabled={authBusy !== null}
            autoComplete="username"
            onChange={(e) => setUsername(e.target.value)}
          />
          <input
            className={AUTH_INPUT_CLASS}
            id="login-password"
            name="password"
            placeholder="Password"
            type="password"
            value={password}
            disabled={authBusy !== null}
            autoComplete="current-password"
            onChange={(e) => setPassword(e.target.value)}
          />
          {rootInitialized === false ? (
            <input
              className={AUTH_INPUT_CLASS}
              id="login-display-name"
              name="display_name"
              placeholder="Display Name"
              value={displayName}
              disabled={authBusy !== null}
              autoComplete="name"
              onChange={(e) => setDisplayName(e.target.value)}
            />
          ) : null}
          <div className={AUTH_ACTIONS_CLASS}>
            {rootInitialized === false ? (
              <ActionButton
                tone="secondary"
                className={AUTH_SECONDARY_BUTTON_CLASS}
                disabled={authBusy !== null}
                onClick={() => onRegister("root")}
              >
                {authBusy === "register" ? "Bootstrapping..." : "Initialize Root"}
              </ActionButton>
            ) : null}
            <ActionButton
              tone="primary"
              type="submit"
              className={AUTH_PRIMARY_BUTTON_CLASS}
              disabled={authBusy !== null}
            >
              {authBusy === "login" ? "Logging in..." : "Login"}
            </ActionButton>
          </div>
        </form>
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
      </div>

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
    </div>
  );
});
AgentsRootPage.displayName = "AgentsRootPage";
