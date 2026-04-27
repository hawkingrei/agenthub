import React, { Suspense } from "react";
import { AgentsPanel, AgentsPanelProps } from "./agents_panel";
import { AgentsWorkbenchProps } from "./agents_workbench_types";
import { OutputHeader, OutputHeaderProps } from "./output_header";
import { OutputErrorBoundary } from "./output_error_boundary";
import {
  APP_WORKSPACE_RIGHT_CLASS,
  APP_WORKSPACE_ROOT_CLASS,
  APP_WORKSPACE_ROOT_COLLAPSED_CLASS,
  APP_WORKSPACE_SPLITTER_CLASS,
  OUTPUT_BODY_ACP_ROOT_CLASS,
  OUTPUT_BODY_LOADING_CLASS,
  OUTPUT_BODY_ROOT_CLASS,
} from "../ui/tailwind_classes";

const LazyAgentsWorkbench = React.lazy(async () => {
  const module = await import("./agents_workbench");
  return { default: module.AgentsWorkbench };
});

function WorkbenchBodyFallback({ hasAcp }: { hasAcp: boolean }) {
  return (
    <div className={hasAcp ? OUTPUT_BODY_ACP_ROOT_CLASS : OUTPUT_BODY_ROOT_CLASS}>
      <div className={OUTPUT_BODY_LOADING_CLASS}>
        <i className="bi bi-hourglass-split spinner" aria-hidden="true" />
        <div className="label">Loading...</div>
      </div>
    </div>
  );
}

type AgentsRouteShellViewProps = {
  agentsCollapsed: boolean;
  workspaceRef: React.RefObject<HTMLElement | null>;
  workspaceStyle?: React.CSSProperties;
  onAgentsSplitterPointerDown: React.PointerEventHandler<HTMLDivElement>;
  agentsPanelProps: AgentsPanelProps;
  outputHeaderProps: OutputHeaderProps;
  showOutputHeader?: boolean;
  workbenchNode: React.ReactNode;
};

export const AgentsRouteShellView = React.memo(function AgentsRouteShellView({
  agentsCollapsed,
  workspaceRef,
  workspaceStyle,
  onAgentsSplitterPointerDown,
  agentsPanelProps,
  outputHeaderProps,
  showOutputHeader = true,
  workbenchNode,
}: AgentsRouteShellViewProps) {
  return (
    <main
      className={
        agentsCollapsed
          ? APP_WORKSPACE_ROOT_COLLAPSED_CLASS
          : APP_WORKSPACE_ROOT_CLASS
      }
      ref={workspaceRef}
      style={workspaceStyle}
    >
      <AgentsPanel {...agentsPanelProps} />
      {!agentsCollapsed && (
        <div
          className={APP_WORKSPACE_SPLITTER_CLASS}
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize agents sidebar"
          onPointerDown={onAgentsSplitterPointerDown}
        />
      )}
      <div className={APP_WORKSPACE_RIGHT_CLASS}>
        {showOutputHeader ? (
          <div
            className={
              outputHeaderProps.hasAcp ? "max-[720px]:hidden shrink-0" : "shrink-0"
            }
          >
            <OutputHeader {...outputHeaderProps} />
          </div>
        ) : null}
        <div className="flex-1 min-h-0 overflow-hidden relative flex flex-col">
          {workbenchNode}
        </div>
      </div>
    </main>
  );
});
AgentsRouteShellView.displayName = "AgentsRouteShellView";

type AgentsRouteShellProps = Omit<AgentsRouteShellViewProps, "workbenchNode"> & {
  workbenchProps: AgentsWorkbenchProps | null;
  rootWorkbenchNode?: React.ReactNode;
};

export const AgentsRouteShell = React.memo(function AgentsRouteShell({
  agentsCollapsed,
  workspaceRef,
  workspaceStyle,
  onAgentsSplitterPointerDown,
  agentsPanelProps,
  outputHeaderProps,
  showOutputHeader = true,
  workbenchProps,
  rootWorkbenchNode = null,
}: AgentsRouteShellProps) {
  const workbenchNode = workbenchProps ? (
    <OutputErrorBoundary>
      <Suspense
        fallback={<WorkbenchBodyFallback hasAcp={workbenchProps.acpView.hasAcp} />}
      >
        <LazyAgentsWorkbench {...workbenchProps} />
      </Suspense>
    </OutputErrorBoundary>
  ) : rootWorkbenchNode;

  return (
    <AgentsRouteShellView
      agentsCollapsed={agentsCollapsed}
      workspaceRef={workspaceRef}
      workspaceStyle={workspaceStyle}
      onAgentsSplitterPointerDown={onAgentsSplitterPointerDown}
      agentsPanelProps={agentsPanelProps}
      outputHeaderProps={outputHeaderProps}
      showOutputHeader={showOutputHeader}
      workbenchNode={workbenchNode}
    />
  );
});
AgentsRouteShell.displayName = "AgentsRouteShell";
