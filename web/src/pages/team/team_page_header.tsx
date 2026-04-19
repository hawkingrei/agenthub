import React from "react";
import { WorkspaceShellHeader, type WorkspaceShellLensItem } from "../../components/workspace_shell_header";

type TeamPageHeaderProps = {
  isSelectorRoute: boolean;
  teamsSidebarCollapsed: boolean;
  teamPanelToggleLabel: string;
  username: string;
  isRoot: boolean;
  headerShellClassName: string;
  headerIconButtonClassName: string;
  lensItems: readonly WorkspaceShellLensItem[];
  onToggleSidebar: () => void;
  onSelectLens: (value: string) => void;
  onNavigate: (pathname: string) => void;
  onLogout: () => void;
};

export const TeamPageHeader = React.memo(function TeamPageHeader({
  isSelectorRoute,
  teamsSidebarCollapsed,
  teamPanelToggleLabel,
  username,
  isRoot,
  headerShellClassName,
  headerIconButtonClassName,
  lensItems,
  onToggleSidebar,
  onSelectLens,
  onNavigate,
  onLogout,
}: TeamPageHeaderProps) {
  return (
    <WorkspaceShellHeader
      activeSurface="teams"
      title={isSelectorRoute ? "Teams" : "Workspace"}
      subtitle={null}
      sidebarToggleLabel={isSelectorRoute ? null : teamPanelToggleLabel}
      sidebarCollapsed={teamsSidebarCollapsed}
      onToggleSidebar={isSelectorRoute ? null : onToggleSidebar}
      username={username}
      isRoot={isRoot}
      headerShellClassName={headerShellClassName}
      headerIconButtonClassName={headerIconButtonClassName}
      menuButtonClassName={`${headerIconButtonClassName} h-auto w-auto gap-1.5 px-2 sm:px-3`}
      lensItems={isSelectorRoute ? [] : lensItems}
      onSelectLens={isSelectorRoute ? null : onSelectLens}
      onNavigate={onNavigate}
      onLogout={onLogout}
    />
  );
});
