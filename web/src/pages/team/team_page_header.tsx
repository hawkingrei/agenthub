import React from "react";
import { ActionButton } from "../../ui/primitives";
import type { ConnectionBadge } from "../../connection_status";
import { WorkspaceShellHeader, type WorkspaceShellLensItem } from "../../components/workspace_shell_header";

type TeamPageHeaderProps = {
  isSelectorRoute: boolean;
  teamsSidebarCollapsed: boolean;
  teamPanelToggleLabel: string;
  connectionBadge: ConnectionBadge;
  username: string;
  isRoot: boolean;
  headerShellClassName: string;
  headerIconButtonClassName: string;
  headerMutedButtonClassName: string;
  headerStatusClassName: string;
  lensItems: readonly WorkspaceShellLensItem[];
  onToggleSidebar: () => void;
  onSelectLens: (value: string) => void;
  onNavigateToSelector: () => void;
  onNavigate: (pathname: string) => void;
  onLogout: () => void;
};

export const TeamPageHeader = React.memo(function TeamPageHeader({
  isSelectorRoute,
  teamsSidebarCollapsed,
  teamPanelToggleLabel,
  connectionBadge,
  username,
  isRoot,
  headerShellClassName,
  headerIconButtonClassName,
  headerMutedButtonClassName,
  headerStatusClassName,
  lensItems,
  onToggleSidebar,
  onSelectLens,
  onNavigateToSelector,
  onNavigate,
  onLogout,
}: TeamPageHeaderProps) {
  return (
    <WorkspaceShellHeader
      activeSurface="teams"
      title={isSelectorRoute ? "Teams" : "Workspace"}
      subtitle={isSelectorRoute ? "Choose a team" : null}
      sidebarToggleLabel={isSelectorRoute ? null : teamPanelToggleLabel}
      sidebarCollapsed={teamsSidebarCollapsed}
      onToggleSidebar={isSelectorRoute ? null : onToggleSidebar}
      connectionBadge={connectionBadge}
      username={username}
      isRoot={isRoot}
      headerShellClassName={headerShellClassName}
      headerIconButtonClassName={headerIconButtonClassName}
      headerStatusClassName={headerStatusClassName}
      menuButtonClassName={`${headerIconButtonClassName} h-auto w-auto gap-1.5 px-2 sm:px-3`}
      secondaryActions={
        !isSelectorRoute ? (
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            className={headerMutedButtonClassName}
            onClick={onNavigateToSelector}
          >
            <i className="bi bi-grid-3x3-gap" aria-hidden="true" />
            Teams
          </ActionButton>
        ) : null
      }
      lensItems={isSelectorRoute ? [] : lensItems}
      onSelectLens={isSelectorRoute ? null : onSelectLens}
      onNavigate={onNavigate}
      onLogout={onLogout}
    />
  );
});
