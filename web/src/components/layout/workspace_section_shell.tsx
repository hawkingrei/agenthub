import React from "react";
import { cx } from "../../ui/primitives";

const WORKSPACE_SECTION_SHELL_BASE_CLASS =
  "min-w-0 rounded-xl border border-notion-border bg-white shadow-sm";
const WORKSPACE_CONTENT_STACK_BASE_CLASS =
  "flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden";
const WORKSPACE_SPLIT_WITH_CONTEXT_CLASS =
  "lg:grid lg:grid-cols-[minmax(0,1.45fr)_minmax(20rem,0.9fr)]";

type WorkspaceSectionShellProps = React.ComponentPropsWithoutRef<"div"> & {
  compact?: boolean;
};

export const WorkspaceSectionShell = React.memo(function WorkspaceSectionShell({
  className,
  compact = false,
  ...props
}: WorkspaceSectionShellProps) {
  return (
    <div
      className={cx(WORKSPACE_SECTION_SHELL_BASE_CLASS, compact ? "py-0.5" : null, className)}
      data-workspace-section-shell="true"
      {...props}
    />
  );
});

WorkspaceSectionShell.displayName = "WorkspaceSectionShell";

type WorkspaceContentStackProps = React.ComponentPropsWithoutRef<"div"> & {
  compact?: boolean;
  gap?: "compact" | "normal" | "tight";
};

export const WorkspaceContentStack = React.memo(function WorkspaceContentStack({
  className,
  compact = false,
  gap,
  ...props
}: WorkspaceContentStackProps) {
  const resolvedGap = gap ?? (compact ? "compact" : "normal");
  const gapClassName =
    resolvedGap === "compact" ? "gap-2" : resolvedGap === "tight" ? "gap-3" : "gap-4";
  return (
    <div
      className={cx(WORKSPACE_CONTENT_STACK_BASE_CLASS, gapClassName, className)}
      data-workspace-content-stack="true"
      {...props}
    />
  );
});

WorkspaceContentStack.displayName = "WorkspaceContentStack";

type WorkspaceSplitPaneLayoutProps = Omit<React.ComponentPropsWithoutRef<"div">, "children"> & {
  primary: React.ReactNode;
  secondary?: React.ReactNode;
  primaryClassName?: string;
  secondaryClassName?: string;
};

export const WorkspaceSplitPaneLayout = React.memo(function WorkspaceSplitPaneLayout({
  className,
  primary,
  secondary = null,
  primaryClassName,
  secondaryClassName,
  ...props
}: WorkspaceSplitPaneLayoutProps) {
  const hasSecondary = Boolean(secondary);
  return (
    <div
      className={cx(
        "flex min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-hidden",
        hasSecondary ? WORKSPACE_SPLIT_WITH_CONTEXT_CLASS : null,
        className
      )}
      data-workspace-split-pane-layout="true"
      data-secondary-open={hasSecondary ? "true" : "false"}
      {...props}
    >
      <div className={cx("min-h-0 min-w-0 flex-1 overflow-hidden", primaryClassName)}>
        {primary}
      </div>
      {hasSecondary ? (
        <div
          className={cx(
            "flex max-h-[40vh] min-h-0 w-full shrink-0 flex-col overflow-hidden lg:h-full lg:max-h-none lg:min-w-0",
            secondaryClassName
          )}
        >
          {secondary}
        </div>
      ) : null}
    </div>
  );
});

WorkspaceSplitPaneLayout.displayName = "WorkspaceSplitPaneLayout";
