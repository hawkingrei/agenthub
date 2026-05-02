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
  onPrefetch?: () => void;
};

type WorkspaceShellHeaderProps = {
  activeSurface: "workspace" | "teams";
  title: string | null;
  subtitle: string | null;
  sidebarToggleLabel?: string | null;
  sidebarCollapsed?: boolean;
  onToggleSidebar?: (() => void) | null;
  username: string;
  isRoot: boolean;
  headerShellClassName: string;
  headerIconButtonClassName: string;
  menuButtonClassName: string;
  connectionBadge?: ConnectionBadge | null;
  headerStatusClassName?: string | null;
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
      username,
      isRoot,
      headerShellClassName: className,
      headerIconButtonClassName,
      menuButtonClassName,
      connectionBadge = null,
      headerStatusClassName = null,
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
        <div
          className="flex min-w-0 flex-1 items-center gap-3"
          data-workspace-shell-primary="true"
        >
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
            <div className="flex min-w-0 flex-1 items-baseline gap-1.5">
              {title ? (
                <h1 className="truncate text-[14px] font-semibold tracking-tight text-black/85">
                  {title}
                </h1>
              ) : null}
              {subtitle ? (
                <p className="truncate text-[12px] font-medium text-black/45">{subtitle}</p>
              ) : null}
            </div>
          )}
        </div>

        <div
          className="flex shrink-0 items-center justify-end gap-3 max-md:order-2 max-md:w-full max-md:justify-between max-md:gap-2"
          data-workspace-shell-actions="true"
        >
          {connectionBadge && headerStatusClassName ? (
            <WorkbenchConnectionBadge
              badge={connectionBadge}
              className={headerStatusClassName}
            />
          ) : null}
          {secondaryActions}
          <WorkbenchHeaderMenu
            active={activeSurface}
            username={username}
            isRoot={isRoot}
            onLogout={onLogout}
            onNavigate={onNavigate}
            buttonClassName={menuButtonClassName}
          />
        </div>

        {lensItems.length > 0 && onSelectLens ? (
          <div
            className={`${WORKSPACE_SHELL_LENS_BAR_CLASS} min-w-0 overflow-x-auto max-md:order-3 max-md:basis-full md:order-2 md:ml-2 md:flex-1`}
            data-workspace-shell-lenses="true"
          >
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
                onMouseEnter={item.onPrefetch}
                onFocus={item.onPrefetch}
                disabled={item.disabled}
                aria-pressed={item.active}
                title={item.title}
              >
                {item.label}
              </ActionButton>
            ))}
          </div>
        ) : null}
      </header>
    );
  })
);

WorkspaceShellHeader.displayName = "WorkspaceShellHeader";
