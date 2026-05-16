import React from "react";
import { HoverCard, Menu, Tooltip } from "@mantine/core";
import { NOTION_FLOATING_MENU_PROPS } from "../../ui/floating_surfaces";
import { DeterministicAvatar } from "../../components/deterministic_avatar";
import { ActionButton } from "../../ui/primitives";
import { TEAM_SOFT_CHROME_SHADOW_CLASS } from "../../ui/tailwind_classes";
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
  const runtimeDotClassNameByColor: Record<TeamRuntimeControlTone["countColor"], string> = {
    teal: "bg-teal-500",
    yellow: "bg-yellow-500",
    gray: "bg-slate-400",
  };
  const runtimeDotClassName =
    runtimeDotClassNameByColor[selectedTeamRuntimeControlTone.countColor];
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
  const selectedAgentIdentityDescription =
    selectedAgentSpecDraft?.description?.trim() || "No agent identity description yet.";

  return (
    <div className={`flex flex-col ${isAgentWorkspace ? "gap-1.5" : "gap-2"}`}>
      <div
        className={`flex min-w-0 flex-nowrap items-center justify-between ${
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
        </div>
        <div className="flex shrink-0 items-center justify-end gap-1.5">
          {isAgentWorkspace ? (
            <HoverCard
              width={300}
              position="bottom-start"
              openDelay={120}
              closeDelay={80}
              shadow="md"
              withArrow
            >
              <Menu position="bottom-end" {...NOTION_FLOATING_MENU_PROPS}>
                <Menu.Target>
                  <HoverCard.Target>
                    <ActionButton
                      type="button"
                      tone="ghost"
                      size="sm"
                      className={`${chrome.mutedButtonClassName} ${chrome.headerActionButtonClassName} inline-flex max-w-[12rem] items-center gap-1.5 rounded-xl border border-black/6 bg-white/88 px-2 py-1 text-left text-[11px] font-medium shadow-notion-row backdrop-blur-[2px] hover:border-black/10 hover:bg-white sm:max-w-full`}
                      aria-label="Agent"
                      title={selectedAgentIdentityDescription}
                    >
                      <DeterministicAvatar
                        name={selectedAgentLabel}
                        stableId={selectedAgentWorkspaceMemberId}
                        className="h-5 w-5 border border-sky-200/80"
                      />
                      <span className="min-w-0 flex items-center gap-1.5">
                        <span className="max-w-[7rem] truncate text-[12px] font-semibold leading-5 text-notion-text sm:max-w-[11rem]">
                          {selectedAgentLabel}
                        </span>
                        <span className="rounded-md bg-notion-sidebar/80 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-notion-text-muted">
                          {selectedAgentStatusView.role}
                        </span>
                      </span>
                      <i className="bi bi-chevron-down shrink-0 text-[9px] text-black/40" aria-hidden="true" />
                    </ActionButton>
                  </HoverCard.Target>
                </Menu.Target>
                  <Menu.Dropdown>
                    <Menu.Label>{selectedAgentLabel}</Menu.Label>
                    {agentWorkspaceSummaryItems.map((item) => (
                      <Menu.Item key={item.label} disabled>
                        <div className="min-w-[220px] text-[12px] leading-5 text-ui-text-secondary">
                          <span className="font-semibold text-ui-text-primary">{item.label}</span>
                          <span className="ml-2">{item.value}</span>
                        </div>
                      </Menu.Item>
                    ))}
                    <Menu.Item disabled>
                      <div className="min-w-[220px] text-[12px] leading-5 text-ui-text-secondary">
                        <span className="font-semibold text-ui-text-primary">Identity</span>
                        <p className="mt-1 whitespace-pre-wrap text-ui-text-secondary">
                          {selectedAgentIdentityDescription}
                        </p>
                      </div>
                    </Menu.Item>
                    <Menu.Item disabled>
                      <div className="min-w-[220px] text-[12px] leading-5 text-ui-text-secondary">
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
              <HoverCard.Dropdown className="rounded-2xl border border-black/6 bg-white/95 p-2.5 shadow-[0_12px_40px_rgba(15,23,42,0.12)] backdrop-blur-md">
                <div className="flex min-w-0 items-start gap-2.5">
                  <DeterministicAvatar
                    name={selectedAgentLabel}
                    stableId={selectedAgentWorkspaceMemberId}
                    className="h-7 w-7 border border-sky-200/80"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="truncate text-[13px] font-semibold text-notion-text">
                        {selectedAgentLabel}
                      </span>
                      <span className="text-[10px] font-semibold uppercase tracking-[0.16em] text-black/35">
                        {selectedAgentStatusView.role}
                      </span>
                    </div>
                    <p className="mt-0.5 text-[12px] leading-5 text-black/60">
                      {selectedAgentIdentityDescription}
                    </p>
                    <p className="mt-1.5 text-[11px] leading-5 text-black/45">
                      {selectedAgentStatusView.currentWork}
                    </p>
                  </div>
                </div>
              </HoverCard.Dropdown>
            </HoverCard>
          ) : showWorkspaceRuntimeBadge ? (
            <Tooltip
              label={`${selectedTeamRuntimeOnline}/${selectedTeamRuntimeTotal} members online`}
              withArrow
            >
              <div
                className={`hidden items-center gap-1.5 rounded-md px-1.5 py-1 text-[11px] text-notion-text-muted sm:inline-flex ${TEAM_SOFT_CHROME_SHADOW_CLASS}`}
              >
                <span className={`inline-flex h-1.5 w-1.5 rounded-full ${runtimeDotClassName}`} aria-hidden="true" />
                <span>{selectedTeamRuntimeStatusLabel}</span>
                <span className="text-notion-text-muted/60">{`${selectedTeamRuntimeOnline}/${selectedTeamRuntimeTotal}`}</span>
              </div>
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
                        ? `${chrome.toolbarButtonActiveClassName} h-6.5 rounded-md px-2 text-[11px] font-medium`
                        : `${chrome.toolbarButtonIdleClassName} h-6.5 rounded-md px-2 text-[11px] font-medium`
                    }
                    aria-label="More"
                    title="More"
                  >
                    <span className="text-black/60">More</span>
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
      {workspaceNoticeText && (
        <div className={chrome.noticeClassName}>
          <div className={chrome.noticeTextClassName}>
            <span className={workspaceNoticeDotClassName} aria-hidden="true" />
            <span className="min-w-0 flex-1 text-[11px] leading-5 text-ui-text-muted/85">
              {workspaceNoticeText}
            </span>
          </div>
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
