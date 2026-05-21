import React from "react";
import type { WorkspaceLens } from "../../app_route_selection";
import type { TeamDefinitionRecord } from "../../api";
import { WorkspacePanelLoadingFallback } from "../../components/workspace_panel_loading_fallback";
import { ActionButton, EmptyState } from "../../ui/primitives";
import {
  TeamLoadingPanel,
  TeamUnavailablePanel,
} from "../team_workspace_state_panel";
import { TeamWorkspaceHeader } from "./team_workspace_header";
import type { TeamTab } from "./state";

const loadTeamEventsPanel = () => import("../team_events_panel");
const loadTeamMailboxPanel = () => import("../team_mailbox_panel");
const loadTeamMemberConsolePanel = () => import("../team_member_console_panel");
const loadTeamOverviewPanel = () => import("../team_overview_panel");
const loadTeamRunPanel = () => import("../team_run_panel");
const loadTeamSetupPanel = () => import("../team_setup_panel");
const loadTeamStepsPanel = () => import("../team_steps_panel");

const LazyTeamEventsPanel = React.lazy(async () => {
  const module = await loadTeamEventsPanel();
  return { default: module.TeamEventsPanel };
});

const LazyTeamMailboxPanel = React.lazy(async () => {
  const module = await loadTeamMailboxPanel();
  return { default: module.TeamMailboxPanel };
});

const LazyTeamMemberConsolePanel = React.lazy(async () => {
  const module = await loadTeamMemberConsolePanel();
  return { default: module.TeamMemberConsolePanel };
});

const LazyTeamOverviewPanel = React.lazy(async () => {
  const module = await loadTeamOverviewPanel();
  return { default: module.TeamOverviewPanel };
});

const LazyTeamRunPanel = React.lazy(async () => {
  const module = await loadTeamRunPanel();
  return { default: module.TeamRunPanel };
});

const LazyTeamSetupPanel = React.lazy(async () => {
  const module = await loadTeamSetupPanel();
  return { default: module.TeamSetupPanel };
});

const LazyTeamStepsPanel = React.lazy(async () => {
  const module = await loadTeamStepsPanel();
  return { default: module.TeamStepsPanel };
});

export function prefetchTeamWorkbenchTab(tab: TeamTab): void {
  switch (tab) {
    case "runs":
      void loadTeamRunPanel();
      return;
    case "overview":
      void loadTeamOverviewPanel();
      return;
    case "events":
      void loadTeamEventsPanel();
      return;
    case "steps":
      void loadTeamStepsPanel();
      return;
    case "mailbox":
      void loadTeamMailboxPanel();
      return;
    case "member_console":
      void loadTeamMemberConsolePanel();
      return;
    default:
      return;
  }
}

export function prefetchTeamSetupSurface(): void {
  void loadTeamSetupPanel();
}

export function TeamPanelLoadingFallback() {
  return <WorkspacePanelLoadingFallback />;
}

export type TeamWorkbenchContentProps = {
  showTeamBootstrapLoading: boolean;
  showTeamUnavailable: boolean;
  onBackToSelector: () => void;
  selectedTeam: TeamDefinitionRecord | null;
  isAgentWorkspace: boolean;
  teamSectionCardClassName: string;
  panelSecondaryButtonClassName: string;
  teamWorkbenchWorkspaceShellClassName: string;
  workspaceHeaderProps: React.ComponentProps<typeof TeamWorkspaceHeader>;
  selectedTeamHasConfiguredMembers: boolean;
  selectedTeamDescription?: string | null;
  teamMemberForgeLabel: string;
  teamMemberCopyExistingLabel: string;
  onOpenTeamMemberForge: () => void;
  onOpenTeamMemberCopyExisting: () => void;
  tab: TeamTab;
  runsPanelProps: React.ComponentProps<typeof import("../team_run_panel").TeamRunPanel>;
  showRunContextLoading: boolean;
  showNoActiveRunNotice: boolean;
  activeWorkspaceLens: WorkspaceLens;
  conversationPanel: React.ReactNode;
  threadPane?: React.ReactNode;
  tasksPanel: React.ReactNode;
  agentAcpPanel: React.ReactNode;
  overviewPanelProps: React.ComponentProps<typeof import("../team_overview_panel").TeamOverviewPanel> | null;
  eventsPanelProps: React.ComponentProps<typeof import("../team_events_panel").TeamEventsPanel> | null;
  stepsPanelProps: React.ComponentProps<typeof import("../team_steps_panel").TeamStepsPanel> | null;
  mailboxHasActiveRun: boolean;
  mailboxEmptyTitle: string;
  mailboxEmptyBody: string;
  onGoToRuns: () => void;
  mailboxPanelProps: React.ComponentProps<typeof import("../team_mailbox_panel").TeamMailboxPanel> | null;
  memberConsolePanelProps: React.ComponentProps<typeof import("../team_member_console_panel").TeamMemberConsolePanel> | null;
  debugPanel: React.ReactNode;
};

export const TeamWorkbenchContent = React.memo(function TeamWorkbenchContent({
  showTeamBootstrapLoading,
  showTeamUnavailable,
  onBackToSelector,
  selectedTeam,
  isAgentWorkspace,
  teamSectionCardClassName,
  panelSecondaryButtonClassName,
  teamWorkbenchWorkspaceShellClassName,
  workspaceHeaderProps,
  selectedTeamHasConfiguredMembers,
  selectedTeamDescription,
  teamMemberForgeLabel,
  teamMemberCopyExistingLabel,
  onOpenTeamMemberForge,
  onOpenTeamMemberCopyExisting,
  tab,
  runsPanelProps,
  showRunContextLoading,
  showNoActiveRunNotice,
  activeWorkspaceLens,
  conversationPanel,
  threadPane = null,
  tasksPanel,
  agentAcpPanel,
  overviewPanelProps,
  eventsPanelProps,
  stepsPanelProps,
  mailboxHasActiveRun,
  mailboxEmptyTitle,
  mailboxEmptyBody,
  onGoToRuns,
  mailboxPanelProps,
  memberConsolePanelProps,
  debugPanel,
}: TeamWorkbenchContentProps) {
  const effectiveWorkspaceLens =
    activeWorkspaceLens === "search" ? "channels" : activeWorkspaceLens;
  const showWorkspaceHeader = effectiveWorkspaceLens !== "tasks" && tab !== "tasks";

  return (
    <div
      className="teams-main flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-hidden pb-3 pr-1 lg:w-full lg:pr-0"
      data-team-surface="workbench"
    >
      {showTeamBootstrapLoading && <TeamLoadingPanel />}

      {showTeamUnavailable && <TeamUnavailablePanel onBackToSelector={onBackToSelector} />}

      {selectedTeam && (
        <div
          className={`flex min-h-0 flex-1 flex-col overflow-hidden ${
            isAgentWorkspace ? "gap-2" : "gap-4"
          }`}
        >
          {showWorkspaceHeader && (
            <div
              className={`${teamWorkbenchWorkspaceShellClassName} ${
                isAgentWorkspace ? "py-0.5" : ""
              }`}
              data-team-workspace-header-shell="true"
            >
              <TeamWorkspaceHeader {...workspaceHeaderProps} />
            </div>
          )}

          {!selectedTeamHasConfiguredMembers && (
            <React.Suspense fallback={<TeamPanelLoadingFallback />}>
              <LazyTeamSetupPanel
                description={selectedTeamDescription}
                forgeLabel={teamMemberForgeLabel}
                copyExistingLabel={teamMemberCopyExistingLabel}
                onForge={onOpenTeamMemberForge}
                onCopyExisting={onOpenTeamMemberCopyExisting}
              />
            </React.Suspense>
          )}

          {tab === "runs" && (
            <React.Suspense fallback={<TeamPanelLoadingFallback />}>
              <LazyTeamRunPanel {...runsPanelProps} />
            </React.Suspense>
          )}

          {showRunContextLoading && (
            <WorkspacePanelLoadingFallback
              className={teamSectionCardClassName}
              title="Loading run context..."
              body="AgentHub is loading the selected team's execution context."
            />
          )}

          {showNoActiveRunNotice && (
            <EmptyState
              className={teamSectionCardClassName}
              title="No Active Execution Run"
              body="Select an existing execution run or start one in the Execution Runs tab before opening this panel."
            >
              <div className="mt-3">
                <ActionButton
                  tone="secondary"
                  size="md"
                  className={panelSecondaryButtonClassName}
                  onClick={onGoToRuns}
                >
                  Go to Execution Runs
                </ActionButton>
              </div>
            </EmptyState>
          )}

          {tab !== "runs" && !showRunContextLoading && !showNoActiveRunNotice && (
            <div
              className={`flex min-h-0 min-w-0 flex-1 flex-col ${
                isAgentWorkspace ? "gap-2" : "gap-3"
              }`}
            >
              {tab === "conversation" && (
                <div
                  className={
                    threadPane
                      ? "flex min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-hidden lg:grid lg:grid-cols-[minmax(0,1fr)_minmax(22rem,1fr)]"
                      : "flex min-h-0 min-w-0 flex-1 flex-col gap-3 overflow-hidden"
                  }
                  data-team-surface="channel-thread-layout"
                  data-thread-open={threadPane ? "true" : "false"}
                >
                  <div
                    className="min-h-0 min-w-0 flex-1 overflow-hidden"
                    data-team-surface="channel-pane"
                  >
                    {conversationPanel}
                  </div>
                  {threadPane && (
                    <div
                      className="flex max-h-[40vh] min-h-0 w-full shrink-0 flex-col overflow-hidden lg:h-full lg:max-h-none lg:min-w-0"
                      data-team-surface="thread-dock"
                    >
                      {threadPane}
                    </div>
                  )}
                </div>
              )}

              {tab === "tasks" && tasksPanel}

              {tab === "agent_acp" && agentAcpPanel}

              {tab === "overview" && overviewPanelProps && (
                <React.Suspense fallback={<TeamPanelLoadingFallback />}>
                  <LazyTeamOverviewPanel {...overviewPanelProps} />
                </React.Suspense>
              )}

              {tab === "events" && eventsPanelProps && (
                <React.Suspense fallback={<TeamPanelLoadingFallback />}>
                  <LazyTeamEventsPanel {...eventsPanelProps} />
                </React.Suspense>
              )}

              {tab === "steps" && stepsPanelProps && (
                <React.Suspense fallback={<TeamPanelLoadingFallback />}>
                  <LazyTeamStepsPanel {...stepsPanelProps} />
                </React.Suspense>
              )}

              {tab === "mailbox" && !mailboxHasActiveRun && (
                <EmptyState
                  className={teamSectionCardClassName}
                  title={mailboxEmptyTitle}
                  body={mailboxEmptyBody}
                >
                  <div className="mt-3">
                    <ActionButton
                      tone="secondary"
                      size="md"
                      className={panelSecondaryButtonClassName}
                      onClick={onGoToRuns}
                    >
                      Go to Execution Runs
                    </ActionButton>
                  </div>
                </EmptyState>
              )}

              {tab === "mailbox" && mailboxHasActiveRun && mailboxPanelProps && (
                <React.Suspense fallback={<TeamPanelLoadingFallback />}>
                  <LazyTeamMailboxPanel {...mailboxPanelProps} />
                </React.Suspense>
              )}

              {tab === "member_console" && memberConsolePanelProps && (
                <React.Suspense fallback={<TeamPanelLoadingFallback />}>
                  <LazyTeamMemberConsolePanel {...memberConsolePanelProps} />
                </React.Suspense>
              )}

              {tab === "debug" && debugPanel}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
