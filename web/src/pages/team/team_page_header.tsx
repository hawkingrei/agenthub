import React from "react";
import type { ConnectionBadge } from "../../connection_status";
import { WorkbenchConnectionBadge } from "../../components/workbench_connection_badge";
import { WorkbenchHeaderMenu } from "../../components/workbench_header_menu";
import { ActionButton, IconButton } from "../../ui/primitives";

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
  onToggleSidebar: () => void;
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
  onToggleSidebar,
  onNavigateToSelector,
  onNavigate,
  onLogout,
}: TeamPageHeaderProps) {
  return (
    <header className={headerShellClassName}>
      <div className="flex min-w-0 items-center gap-3">
        {!isSelectorRoute && (
          <IconButton
            className={headerIconButtonClassName}
            onClick={onToggleSidebar}
            title={teamPanelToggleLabel}
            aria-label={teamPanelToggleLabel}
          >
            <i
              className={teamsSidebarCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"}
              aria-hidden="true"
            />
          </IconButton>
        )}
        {isSelectorRoute ? (
          <div className="min-w-0">
            <h1 className="text-[clamp(1.1rem,1.85vw,1.45rem)] font-semibold leading-[1.05] tracking-tight text-black">
              Team Selector
            </h1>
            <p className="mt-0.5 text-[12px] text-black/55">Choose a team</p>
          </div>
        ) : null}
      </div>
      <div className="flex flex-wrap items-center justify-end gap-2">
        {!isSelectorRoute && (
          <ActionButton
            type="button"
            tone="secondary"
            size="sm"
            className={headerMutedButtonClassName}
            onClick={onNavigateToSelector}
          >
            <i className="bi bi-grid-3x3-gap" aria-hidden="true" />
            Team Selector
          </ActionButton>
        )}
        <WorkbenchConnectionBadge badge={connectionBadge} className={headerStatusClassName} />
        <WorkbenchHeaderMenu
          active="teams"
          username={username}
          isRoot={isRoot}
          onLogout={onLogout}
          onNavigate={onNavigate}
          buttonClassName={`${headerIconButtonClassName} h-auto w-auto gap-1.5 px-2 sm:px-3`}
        />
      </div>
    </header>
  );
});
