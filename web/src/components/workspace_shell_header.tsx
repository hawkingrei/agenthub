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
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <div className="flex min-w-0 items-start gap-3">
            {showSidebarToggle ? (
              <IconButton
                className={headerIconButtonClassName}
                onClick={onToggleSidebar ?? undefined}
                title={sidebarToggleLabel ?? undefined}
                aria-label={sidebarToggleLabel ?? undefined}
              >
                <i
                  className={sidebarCollapsed ? "bi bi-chevron-right" : "bi bi-chevron-left"}
                  aria-hidden="true"
                />
              </IconButton>
            ) : null}
            {(title || subtitle) && (
              <div className="min-w-0">
                {title ? (
                  <h1 className="text-[clamp(1rem,1.45vw,1.2rem)] font-semibold leading-[1.08] tracking-tight text-black">
                    {title}
                  </h1>
                ) : null}
                {subtitle ? (
                  <p className="mt-0.5 text-[11px] text-black/48">{subtitle}</p>
                ) : null}
              </div>
            )}
          </div>
          {lensItems.length > 0 && onSelectLens ? (
            <div className={WORKSPACE_SHELL_LENS_BAR_CLASS}>
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
        <div className="flex flex-wrap items-center justify-end gap-2">
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
