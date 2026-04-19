import React from "react";
import type { WorkspaceLens } from "../../app_route_selection";
import type { TeamDefinitionRecord } from "../../api";
import { ActionButton } from "../../ui/primitives";
import { TeamEventsPanel } from "../team_events_panel";
import { TeamMailboxPanel } from "../team_mailbox_panel";
import { TeamMemberConsolePanel } from "../team_member_console_panel";
import { TeamOverviewPanel } from "../team_overview_panel";
import { TeamRunPanel } from "../team_run_panel";
import { TeamSetupPanel } from "../team_setup_panel";
import { TeamStepsPanel } from "../team_steps_panel";
import {
  TeamLoadingPanel,
  TeamUnavailablePanel,
} from "../team_workspace_state_panel";
import { TeamWorkspaceHeader } from "./team_workspace_header";
import type { TeamTab } from "./state";

type TeamWorkbenchContentProps = {
  showTeamBootstrapLoading: boolean;
  showTeamUnavailable: boolean;
  onBackToSelector: () => void;
  selectedTeam: TeamDefinitionRecord | null;
  isAgentWorkspace: boolean;
  teamSectionCardClassName: string;
  teamSectionHeadingClassName: string;
  teamSectionTitleClassName: string;
  teamSectionBodyTextClassName: string;
  teamSectionHintTextClassName: string;
  panelSecondaryButtonClassName: string;
  teamWorkbenchWorkspaceShellClassName: string;
  workspaceHeaderProps: React.ComponentProps<typeof TeamWorkspaceHeader>;
  selectedTeamHasConfiguredMembers: boolean;
  selectedTeamDescription?: string | null;
  teamMemberForgeLabel: string;
  onOpenTeamMemberForge: () => void;
  tab: TeamTab;
  runsPanelProps: React.ComponentProps<typeof TeamRunPanel>;
  showRunContextLoading: boolean;
  showNoActiveRunNotice: boolean;
  activeWorkspaceLens: WorkspaceLens;
  conversationPanel: React.ReactNode;
  threadPane?: React.ReactNode;
  tasksPanel: React.ReactNode;
  agentAcpPanel: React.ReactNode;
  overviewPanelProps: React.ComponentProps<typeof TeamOverviewPanel> | null;
  eventsPanelProps: React.ComponentProps<typeof TeamEventsPanel> | null;
  stepsPanelProps: React.ComponentProps<typeof TeamStepsPanel> | null;
  mailboxHasActiveRun: boolean;
  mailboxEmptyTitle: string;
  mailboxEmptyBody: string;
  onGoToRuns: () => void;
  mailboxPanelProps: React.ComponentProps<typeof TeamMailboxPanel> | null;
  memberConsolePanelProps: React.ComponentProps<typeof TeamMemberConsolePanel> | null;
  debugPanel: React.ReactNode;
};

export const TeamWorkbenchContent = React.memo(function TeamWorkbenchContent({
  showTeamBootstrapLoading,
  showTeamUnavailable,
  onBackToSelector,
  selectedTeam,
  isAgentWorkspace,
  teamSectionCardClassName,
  teamSectionHeadingClassName,
  teamSectionTitleClassName,
  teamSectionBodyTextClassName,
  teamSectionHintTextClassName,
  panelSecondaryButtonClassName,
  teamWorkbenchWorkspaceShellClassName,
  workspaceHeaderProps,
  selectedTeamHasConfiguredMembers,
  selectedTeamDescription,
  teamMemberForgeLabel,
  onOpenTeamMemberForge,
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
  return (
    <div
      className="teams-main flex min-h-0 min-w-0 flex-1 flex-col gap-4 overflow-hidden pb-3 pr-1 lg:mx-auto lg:w-full lg:max-w-[1180px] lg:pr-0"
      data-team-surface="workbench"
    >
      {showTeamBootstrapLoading && <TeamLoadingPanel />}

      {showTeamUnavailable && <TeamUnavailablePanel onBackToSelector={onBackToSelector} />}

      {selectedTeam && (
        <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden">
          <div
            className={`${teamSectionCardClassName} ${teamWorkbenchWorkspaceShellClassName} ${
              isAgentWorkspace ? "py-0.5" : ""
            }`}
          >
            <TeamWorkspaceHeader {...workspaceHeaderProps} />
          </div>

          {!selectedTeamHasConfiguredMembers && (
            <TeamSetupPanel
              description={selectedTeamDescription}
              forgeLabel={teamMemberForgeLabel}
              onForge={onOpenTeamMemberForge}
            />
          )}

          {tab === "runs" && <TeamRunPanel {...runsPanelProps} />}

          {showRunContextLoading && (
            <div className={teamSectionCardClassName}>
              <p className="text-sm text-ui-text-muted">
                Loading run context for selected team...
              </p>
            </div>
          )}

          {showNoActiveRunNotice && (
            <div className={teamSectionCardClassName}>
              <h3 className={teamSectionTitleClassName}>No Active Execution Run</h3>
              <p className={teamSectionBodyTextClassName}>
                Select an existing execution run or start one in the Execution Runs tab before
                opening this panel.
              </p>
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
            </div>
          )}

          {tab !== "runs" && !showRunContextLoading && !showNoActiveRunNotice && (
            <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
              {activeWorkspaceLens === "search" && (
                <div className={teamSectionCardClassName}>
                  <div className={teamSectionHeadingClassName}>Search</div>
                  <h3 className={teamSectionTitleClassName}>Search is still workspace-local</h3>
                  <p className={teamSectionHintTextClassName}>
                    Use Channels, Tasks, or Members while shared search rollup is being wired in.
                  </p>
                </div>
              )}

              {activeWorkspaceLens !== "search" && tab === "conversation" && (
                <div className="flex min-h-0 min-w-0 flex-1 gap-3">
                  <div className="min-h-0 min-w-0 flex-1">{conversationPanel}</div>
                  {threadPane}
                </div>
              )}

              {activeWorkspaceLens !== "search" && tab === "tasks" && tasksPanel}

              {tab === "agent_acp" && agentAcpPanel}

              {tab === "overview" && overviewPanelProps && (
                <TeamOverviewPanel {...overviewPanelProps} />
              )}

              {tab === "events" && eventsPanelProps && <TeamEventsPanel {...eventsPanelProps} />}

              {tab === "steps" && stepsPanelProps && <TeamStepsPanel {...stepsPanelProps} />}

              {tab === "mailbox" && !mailboxHasActiveRun && (
                <div className={teamSectionCardClassName}>
                  <h3 className={teamSectionTitleClassName}>{mailboxEmptyTitle}</h3>
                  <p className={teamSectionBodyTextClassName}>{mailboxEmptyBody}</p>
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
                </div>
              )}

              {tab === "mailbox" && mailboxHasActiveRun && mailboxPanelProps && (
                <TeamMailboxPanel {...mailboxPanelProps} />
              )}

              {tab === "member_console" && memberConsolePanelProps && (
                <TeamMemberConsolePanel {...memberConsolePanelProps} />
              )}

              {tab === "debug" && debugPanel}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
