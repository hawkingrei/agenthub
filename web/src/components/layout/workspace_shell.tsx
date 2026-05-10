import React from "react";
import type { ConnectionBadge } from "../../connection_status";
import { ErrorBanner } from "../../error_banner";
import { 
  APP_WORKBENCH_HEADER_CLASS, 
  APP_WORKBENCH_SIDEBAR_TOGGLE_BUTTON_CLASS, 
  APP_WORKBENCH_ACCOUNT_MENU_BUTTON_CLASS, 
  APP_WORKBENCH_HEADER_STATUS_CLASS,
  WORKSPACE_CONTENT_ROOT_CLASS,
  WORKSPACE_CONTENT_PADDING_CLASS
} from "../../ui/tailwind_classes";
import { WorkspaceShellHeader, type WorkspaceShellLensItem } from "../workspace_shell_header";

export type WorkspaceShellProps = {
  appRootRef?: React.RefObject<HTMLDivElement | null>;
  headerRef?: React.RefObject<HTMLElement | null>;
  
  // Header Props
  title: string | null;
  subtitle: string | null;
  activeSurface: "workspace" | "teams";
  username: string;
  isRoot: boolean;
  connectionBadge?: ConnectionBadge | null;
  
  // Sidebar/Agents state
  agentsCollapsed: boolean;
  onToggleAgents: () => void;
  sidebarToggleLabel?: string | null;
  
  // Realtime/Global Error
  normalizedError: string | null;
  onClearError: () => void;
  
  // Lens
  lensItems?: readonly WorkspaceShellLensItem[];
  onSelectLens?: (value: string) => void;
  
  // Navigation
  onNavigate: (pathname: string) => void;
  onLogout: () => void;
  
  // Content
  children: React.ReactNode;
  
  // Notices/Alerts
  warningNotice?: React.ReactNode;
  
  // Modals/Overlays
  modals?: React.ReactNode;
};

export const WorkspaceShell = React.memo(function WorkspaceShell({
  appRootRef,
  headerRef,
  title,
  subtitle,
  activeSurface,
  username,
  isRoot,
  connectionBadge,
  agentsCollapsed,
  onToggleAgents,
  normalizedError,
  onClearError,
  lensItems = [],
  onSelectLens,
  onNavigate,
  onLogout,
  children,
  warningNotice,
  modals,
}: WorkspaceShellProps) {
  const defaultSidebarToggleLabel = agentsCollapsed ? "Show agents" : "Hide agents";

  return (
    <div 
      className="flex h-screen flex-col overflow-hidden bg-white" 
      ref={appRootRef as React.Ref<HTMLDivElement>}
    >
      <WorkspaceShellHeader
        ref={headerRef as React.Ref<HTMLElement>}
        activeSurface={activeSurface}
        title={title}
        subtitle={subtitle}
        sidebarToggleLabel={sidebarToggleLabel ?? defaultSidebarToggleLabel}
        sidebarCollapsed={agentsCollapsed}
        onToggleSidebar={onToggleAgents}
        username={username}
        isRoot={isRoot}
        headerShellClassName={APP_WORKBENCH_HEADER_CLASS}
        headerIconButtonClassName={`${APP_WORKBENCH_SIDEBAR_TOGGLE_BUTTON_CLASS} ${
          agentsCollapsed ? "bg-white" : "bg-notion-hover text-notion-text"
        }`}
        menuButtonClassName={APP_WORKBENCH_ACCOUNT_MENU_BUTTON_CLASS}
        connectionBadge={connectionBadge}
        headerStatusClassName={APP_WORKBENCH_HEADER_STATUS_CLASS}
        lensItems={lensItems}
        onSelectLens={onSelectLens}
        onNavigate={onNavigate}
        onLogout={onLogout}
      />

      <div className={WORKSPACE_CONTENT_ROOT_CLASS}>
        {normalizedError ? (
          <div className={WORKSPACE_CONTENT_PADDING_CLASS}>
            <ErrorBanner message={normalizedError} onClose={onClearError} />
          </div>
        ) : null}

        {warningNotice}
        
        {children}
      </div>

      {modals}
    </div>
  );
});

WorkspaceShell.displayName = "WorkspaceShell";
