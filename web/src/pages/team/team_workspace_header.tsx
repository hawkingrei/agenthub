import React from "react";
import { Badge, Group, Menu, Tooltip } from "@mantine/core";
import { NOTION_FLOATING_MENU_PROPS } from "../../ui/floating_surfaces";
import { ActionButton } from "../../ui/primitives";
import { TEAM_SOFT_CHROME_SHADOW_CLASS } from "../../ui/tailwind_classes";
import { TeamTabsBar } from "../team_tabs_bar";
import type { TeamMemberProfileDraft } from "./create_helpers";
import type {
  AgentWorkspaceStatusView,
  TeamMemberAgentControlState,
  TeamRuntimeControlTone,
} from "./page_helpers";
import type { TeamTab } from "./state";

type WorkspaceTabItem = {
  value: TeamTab;
  label: string;
};

type TeamWorkspaceHeaderChrome = {
  mutedButtonClassName: string;
  headerActionButtonClassName: string;
  toolbarClassName: string;
  toolbarButtonActiveClassName: string;
  toolbarButtonIdleClassName: string;
  noticeClassName: string;
  noticeTextClassName: string;
  runMetaItemClassName: string;
};

export const TeamWorkspaceHeader = React.memo(function TeamWorkspaceHeader({
  workspaceEyebrow,
  showDedicatedWorkspaceHeading,
  workspaceTitle,
  workspaceDescription,
  isAgentWorkspace,
  selectedAgentLabel,
  selectedAgentWorkspaceMemberId,
  selectedAgentStatusView,
  selectedAgentSpecDraft,
  selectedAgentControlState,
  showWorkspaceRuntimeBadge,
  selectedTeamRuntimeStatusLabel,
  selectedTeamRuntimeOnline,
  selectedTeamRuntimeTotal,
  selectedTeamRuntimeControlTone,
  workspaceAdvancedTabItems,
  isAdvancedWorkspace,
  showRunActionsInAdvanced,
  activeRunStatus,
  canResumeActiveRun,
  canRestartActiveRun,
  developerMode,
  workspaceDetailsOpen,
  workspaceDetailItems,
  workspaceNoticeText,
  workspaceNoticeDotClassName,
  workflowTabItems,
  tab,
  busy,
  chrome,
  onTabChange,
  onToggleWorkspaceDetails,
  onRefreshActiveRun,
  onCancelRun,
  onResumeRun,
  onRestartRun,
  onOpenTeamMemberEditModal,
  onStartSelectedTeamAgent,
  onStopSelectedTeamAgent,
  onDeleteSelectedTeamAgent,
}: {
  workspaceEyebrow: string | null;
  showDedicatedWorkspaceHeading: boolean;
  workspaceTitle: string;
  workspaceDescription: string | null;
  isAgentWorkspace: boolean;
  selectedAgentLabel: string;
  selectedAgentWorkspaceMemberId: string;
  selectedAgentStatusView: AgentWorkspaceStatusView;
  selectedAgentSpecDraft: TeamMemberProfileDraft | null;
  selectedAgentControlState: TeamMemberAgentControlState;
  showWorkspaceRuntimeBadge: boolean;
  selectedTeamRuntimeStatusLabel: string;
  selectedTeamRuntimeOnline: number;
  selectedTeamRuntimeTotal: number;
  selectedTeamRuntimeControlTone: TeamRuntimeControlTone;
  workspaceAdvancedTabItems: ReadonlyArray<WorkspaceTabItem>;
  isAdvancedWorkspace: boolean;
  showRunActionsInAdvanced: boolean;
  activeRunStatus: string | null;
  canResumeActiveRun: boolean;
  canRestartActiveRun: boolean;
  developerMode: boolean;
  workspaceDetailsOpen: boolean;
  workspaceDetailItems: readonly string[];
  workspaceNoticeText: string | null;
  workspaceNoticeDotClassName: string;
  workflowTabItems: ReadonlyArray<WorkspaceTabItem>;
  tab: TeamTab;
  busy: string | null;
  chrome: TeamWorkspaceHeaderChrome;
  onTabChange: (tab: TeamTab) => void;
  onToggleWorkspaceDetails: () => void;
  onRefreshActiveRun: () => void;
  onCancelRun: () => void;
  onResumeRun: () => void;
  onRestartRun: () => void;
  onOpenTeamMemberEditModal: () => void;
  onStartSelectedTeamAgent: () => void;
  onStopSelectedTeamAgent: () => void;
  onDeleteSelectedTeamAgent: () => void;
}) {
  const agentWorkspaceSummaryItems = [
    {
      label: "Member",
      value: selectedAgentWorkspaceMemberId || "-",
    },
    { label: "Role", value: selectedAgentStatusView.role },
    {
      label: "Lifecycle",
      value: selectedAgentStatusView.lifecycle,
    },
    { label: "Work", value: selectedAgentStatusView.work },
    { label: "Inbox", value: selectedAgentStatusView.inbox },
    {
      label: "Loop",
      value: selectedAgentSpecDraft?.agent_loop_enabled
        ? `${selectedAgentSpecDraft.agent_loop_idle_seconds.trim() || "?"}s idle`
        : "disabled",
    },
  ];

  return (
    <div className={`flex flex-col ${isAgentWorkspace ? "gap-2" : "gap-3"}`}>
      <div
        className={`flex flex-wrap items-start justify-between ${
          isAgentWorkspace ? "gap-1.5" : "gap-2"
        }`}
      >
        <div className="min-w-0 flex-1">
          {workspaceEyebrow && (
            <p className="text-[10px] font-bold uppercase tracking-[0.18em] text-notion-text-muted">
              {workspaceEyebrow}
            </p>
          )}
          {showDedicatedWorkspaceHeading ? (
            <h2
              className={`${workspaceEyebrow ? "mt-0.5" : ""} ${
                isAgentWorkspace
                  ? "text-[15px] font-semibold leading-[1.15]"
                  : "text-[18px] font-semibold leading-tight"
              } tracking-tight text-notion-text`}
            >
              {workspaceTitle}
            </h2>
          ) : null}
          {workspaceDescription && (
            <p className="mt-1 max-w-[64ch] text-[12px] leading-[1.55] text-notion-text-muted">
              {workspaceDescription}
            </p>
          )}
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {isAgentWorkspace ? (
            <Menu position="bottom-end" {...NOTION_FLOATING_MENU_PROPS}>
              <Menu.Target>
                <ActionButton
                  type="button"
                  tone="secondary"
                  size="sm"
                  className={`${chrome.mutedButtonClassName} ${chrome.headerActionButtonClassName} inline-flex items-center gap-1 rounded-md px-2 py-1 text-[11px] font-semibold`}
                  aria-label="Open agent workspace menu"
                >
                  <i className="bi bi-person-badge" aria-hidden="true" />
                  <span>Agent</span>
                </ActionButton>
              </Menu.Target>
              <Menu.Dropdown>
                <Menu.Label>{selectedAgentLabel}</Menu.Label>
                {agentWorkspaceSummaryItems.map((item) => (
                  <Menu.Item key={item.label} disabled>
                    <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                      <span className="font-semibold text-ui-text-primary">{item.label}</span>
                      <span className="ml-2">{item.value}</span>
                    </div>
                  </Menu.Item>
                ))}
                <Menu.Item disabled>
                  <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                    <span className="font-semibold text-ui-text-primary">Identity</span>
                    <p className="mt-1 whitespace-pre-wrap text-ui-text-secondary">
                      {selectedAgentSpecDraft?.description?.trim() ||
                        "No agent identity description yet."}
                    </p>
                  </div>
                </Menu.Item>
                <Menu.Item disabled>
                  <div className="min-w-[240px] text-[12px] leading-5 text-ui-text-secondary">
                    <span className="font-semibold text-ui-text-primary">Current work</span>
                    <p className="mt-1 whitespace-pre-wrap text-ui-text-secondary">
                      {selectedAgentStatusView.currentWork}
                    </p>
                  </div>
                </Menu.Item>
                <Menu.Divider />
                <Menu.Item
                  leftSection={<i className="bi bi-pencil-square" aria-hidden="true" />}
                  onClick={onOpenTeamMemberEditModal}
                >
                  Edit profile
                </Menu.Item>
                <Menu.Item
                  leftSection={<i className="bi bi-play-circle" aria-hidden="true" />}
                  onClick={onStartSelectedTeamAgent}
                  disabled={!selectedAgentControlState.canStart}
                >
                  Start Agent
                </Menu.Item>
                <Menu.Item
                  leftSection={<i className="bi bi-stop-circle" aria-hidden="true" />}
                  onClick={onStopSelectedTeamAgent}
                  disabled={!selectedAgentControlState.canStop}
                >
                  Stop Agent
                </Menu.Item>
                <Menu.Item
                  color="red"
                  leftSection={<i className="bi bi-trash" aria-hidden="true" />}
                  onClick={onDeleteSelectedTeamAgent}
                  disabled={!selectedAgentControlState.canDelete}
                >
                  Delete Agent
                </Menu.Item>
              </Menu.Dropdown>
            </Menu>
          ) : showWorkspaceRuntimeBadge ? (
            <Tooltip
              label={`${selectedTeamRuntimeOnline}/${selectedTeamRuntimeTotal} members online`}
              withArrow
            >
              <Group
                gap={8}
                wrap="nowrap"
                className={`rounded-[10px] border border-notion-border/65 bg-notion-sidebar/55 px-2 py-1 ${TEAM_SOFT_CHROME_SHADOW_CLASS}`}
              >
                <Badge
                  variant="light"
                  color={selectedTeamRuntimeControlTone.statusColor}
                  radius="sm"
                  className="font-semibold"
                >
                  {selectedTeamRuntimeStatusLabel}
                </Badge>
                <Badge
                  variant="dot"
                  color={selectedTeamRuntimeControlTone.countColor}
                  radius="sm"
                  className="font-semibold"
                >
                  {`${selectedTeamRuntimeOnline}/${selectedTeamRuntimeTotal} online`}
                </Badge>
              </Group>
            </Tooltip>
          ) : null}
          <div className={chrome.toolbarClassName}>
            {(workspaceAdvancedTabItems.length > 0 || showRunActionsInAdvanced) && (
              <Menu position="bottom-end" {...NOTION_FLOATING_MENU_PROPS}>
                <Menu.Target>
                  <ActionButton
                    type="button"
                    tone="ghost"
                    size="sm"
                    className={
                      isAdvancedWorkspace
                        ? chrome.toolbarButtonActiveClassName
                        : chrome.toolbarButtonIdleClassName
                    }
                    aria-label="Open more workspace actions"
                  >
                    <i className="bi bi-three-dots" aria-hidden="true" />
                    <span>More</span>
                  </ActionButton>
                </Menu.Target>
                <Menu.Dropdown>
                  {workspaceAdvancedTabItems.length > 0 && (
                    <>
                      <Menu.Label>Views</Menu.Label>
                      {workspaceAdvancedTabItems.map((item) => (
                        <Menu.Item key={item.value} onClick={() => onTabChange(item.value)}>
                          {item.label}
                        </Menu.Item>
                      ))}
                    </>
                  )}
                  {showRunActionsInAdvanced && (
                    <>
                      {workspaceAdvancedTabItems.length > 0 && <Menu.Divider />}
                      <Menu.Label>Run</Menu.Label>
                      <Menu.Item onClick={onRefreshActiveRun}>Refresh Run</Menu.Item>
                      <Menu.Item
                        onClick={onCancelRun}
                        disabled={busy === "cancel-run" || activeRunStatus === "canceled"}
                      >
                        Cancel
                      </Menu.Item>
                      <Menu.Item
                        onClick={onResumeRun}
                        disabled={busy === "resume-run" || !canResumeActiveRun}
                      >
                        Resume
                      </Menu.Item>
                      <Menu.Item
                        onClick={onRestartRun}
                        disabled={busy === "restart-run" || !canRestartActiveRun}
                      >
                        Restart
                      </Menu.Item>
                    </>
                  )}
                  {developerMode && workspaceDetailItems.length > 0 && (
                    <>
                      {(workspaceAdvancedTabItems.length > 0 || showRunActionsInAdvanced) && (
                        <Menu.Divider />
                      )}
                      <Menu.Label>Workspace</Menu.Label>
                      <Menu.Item onClick={onToggleWorkspaceDetails}>
                        {workspaceDetailsOpen
                          ? "Hide workspace details"
                          : "Show workspace details"}
                      </Menu.Item>
                    </>
                  )}
                </Menu.Dropdown>
              </Menu>
            )}
          </div>
        </div>
      </div>
      {!isAgentWorkspace && (
        <TeamTabsBar tab={tab} onTabChange={onTabChange} items={workflowTabItems} />
      )}
      {(workspaceNoticeText || developerMode) && (
        <div className={chrome.noticeClassName}>
          {workspaceNoticeText && (
            <div className={chrome.noticeTextClassName}>
              <span className={workspaceNoticeDotClassName} aria-hidden="true" />
              <span className="min-w-0 flex-1 text-[11px] leading-5 text-ui-text-muted">
                {workspaceNoticeText}
              </span>
            </div>
          )}
        </div>
      )}
      {developerMode && workspaceDetailsOpen && workspaceDetailItems.length > 0 && (
        <div className="mt-1 flex flex-wrap gap-2">
          {workspaceDetailItems.map((item) => (
            <div key={item} className={chrome.runMetaItemClassName}>
              {item}
            </div>
          ))}
        </div>
      )}
    </div>
  );
});
