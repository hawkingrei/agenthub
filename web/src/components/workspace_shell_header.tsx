import React from "react";
import type { ConnectionBadge } from "../connection_status";
import {
  WORKSPACE_SHELL_LENS_BAR_CLASS,
  WORKSPACE_SHELL_LENS_BUTTON_ACTIVE_CLASS,
  WORKSPACE_SHELL_LENS_BUTTON_IDLE_CLASS,
} from "../ui/tailwind_classes";
import { ActionButton, IconButton } from "../ui/primitives";
import { WorkbenchConnectionBadge } from "./workbench_connection_badge";
import { WorkbenchHeaderMenu } from "./workbench_header_menu";

export type WorkspaceShellLensItem = {
  value: string;
  label: string;
  active: boolean;
  disabled?: boolean;
  title?: string;
};

type WorkspaceShellHeaderProps = {
  activeSurface: "workspace" | "teams";
  title: string | null;
  subtitle: string | null;
  sidebarToggleLabel?: string | null;
  sidebarCollapsed?: boolean;
  onToggleSidebar?: (() => void) | null;
  connectionBadge: ConnectionBadge;
  username: string;
  isRoot: boolean;
  headerShellClassName: string;
  headerIconButtonClassName: string;
  headerStatusClassName: string;
  menuButtonClassName: string;
  secondaryActions?: React.ReactNode;
  lensItems?: readonly WorkspaceShellLensItem[];
  onSelectLens?: ((value: string) => void) | null;
  onNavigate: (pathname: string) => void;
  onLogout: () => void;
};

export const WorkspaceShellHeader = React.memo(
  React.forwardRef<HTMLElement, WorkspaceShellHeaderProps>(function WorkspaceShellHeader(
    {
      activeSurface,
      title,
      subtitle,
      sidebarToggleLabel = null,
      sidebarCollapsed = false,
      onToggleSidebar = null,
      connectionBadge,
      username,
      isRoot,
      headerShellClassName: className,
      headerIconButtonClassName,
      headerStatusClassName,
      menuButtonClassName,
      secondaryActions = null,
      lensItems = [],
      onSelectLens = null,
      onNavigate,
      onLogout,
    },
    ref
  ) {
    const showSidebarToggle = Boolean(sidebarToggleLabel && onToggleSidebar);

    return (
      <header className={className} ref={ref}>
        <div className="flex min-w-0 flex-1 items-center gap-4">
          <div className="flex min-w-0 items-center gap-2">
            {showSidebarToggle ? (
              <IconButton
                className={headerIconButtonClassName}
                onClick={onToggleSidebar ?? undefined}
                title={sidebarToggleLabel ?? undefined}
                aria-label={sidebarToggleLabel ?? undefined}
              >
                <i
                  className={sidebarCollapsed ? "bi bi-layout-sidebar" : "bi bi-layout-sidebar-inset"}
                  aria-hidden="true"
                />
              </IconButton>
            ) : null}
            {(title || subtitle) && (
              <div className="flex min-w-0 items-center gap-1.5">
                {title ? (
                  <h1 className="truncate text-[14px] font-semibold tracking-tight text-black/85">
                    {title}
                  </h1>
                ) : null}
                {title && subtitle ? (
                  <span className="text-black/15 font-normal" aria-hidden="true">/</span>
                ) : null}
                {subtitle ? (
                  <p className="truncate text-[13px] font-medium text-black/45">{subtitle}</p>
                ) : null}
              </div>
            )}
          </div>

          {lensItems.length > 0 && onSelectLens ? (
            <div className={`${WORKSPACE_SHELL_LENS_BAR_CLASS} hidden md:flex ml-2`}>
              {lensItems.map((item) => (
                <ActionButton
                  key={item.value}
                  type="button"
                  tone={item.active ? "secondary" : "ghost"}
                  size="sm"
                  className={
                    item.active
                      ? WORKSPACE_SHELL_LENS_BUTTON_ACTIVE_CLASS
                      : WORKSPACE_SHELL_LENS_BUTTON_IDLE_CLASS
                  }
                  onClick={() => onSelectLens(item.value)}
                  disabled={item.disabled}
                  aria-pressed={item.active}
                  title={item.title}
                >
                  {item.label}
                </ActionButton>
              ))}
            </div>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center justify-end gap-3">

          {secondaryActions}
          <WorkbenchConnectionBadge badge={connectionBadge} className={headerStatusClassName} />
          <WorkbenchHeaderMenu
            active={activeSurface}
            username={username}
            isRoot={isRoot}
            onLogout={onLogout}
            onNavigate={onNavigate}
            buttonClassName={menuButtonClassName}
          />
        </div>
      </header>
    );
  })
);

WorkspaceShellHeader.displayName = "WorkspaceShellHeader";
