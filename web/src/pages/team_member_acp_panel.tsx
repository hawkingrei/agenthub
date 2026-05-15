import React from "react";
import { isAgentActiveStatus } from "../agent_ws";
import { AgentEvent, TeamMemberSnapshot } from "../api";
import { getTeamStepRuntimeHandleId } from "../api";
import {
  buildWorkspaceNodePath,
  navigateToPath,
  shouldHandleInAppLinkClick,
} from "../app_route_selection";
import {
  AcpPanel,
} from "../components/acp_panel";
import { OutputHeaderDetails } from "../components/output_header";
import { InputDock } from "../components/input_dock";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  isActiveTeamMemberStatus,
  normalizeTeamMemberStatusValue,
  TeamMemberAcpTab,
  useTeamMemberAcpViewModel,
} from "./team/use_team_member_acp_view_model";
import { useTeamMemberAcpInput } from "./team/use_team_member_acp_input";
import { useTeamMemberAcpPanelState } from "./team/use_team_member_acp_panel_state";
import { Badge } from "../ui/primitives";
import {
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_HEADING_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
  OUTPUT_HEADER_TITLE_TEXT_CLASS,
  TEAM_MEMBER_ACP_ACTIVITY_BADGE_CLASS,
  TEAM_MEMBER_ACP_ACTIVITY_STRIP_CLASS,
  TEAM_MEMBER_ACP_BODY_SHELL_CLASS,
  TEAM_MEMBER_ACP_DESCRIPTION_CLASS,
  TEAM_MEMBER_ACP_HEADER_CONTEXT_CLASS,
  TEAM_MEMBER_ACP_INFO_BADGE_CLASS,
  TEAM_MEMBER_ACP_INFO_STRIP_CLASS,
  TEAM_MEMBER_ACP_INPUT_DOCK_SHELL_CLASS,
  TEAM_MEMBER_ACP_NODE_LINK_CLASS,
  TEAM_MEMBER_ACP_PANEL_BODY_CLASS,
  TEAM_MEMBER_ACP_PANEL_SHELL_CLASS,
  TEAM_MEMBER_ACP_SENDING_TEXT_CLASS,
  TEAM_MEMBER_ACP_STARTING_LABEL_CLASS,
  TEAM_MEMBER_ACP_STARTING_PROGRESS_BAR_CLASS,
  TEAM_MEMBER_ACP_STARTING_PROGRESS_TRACK_CLASS,
  TEAM_MEMBER_ACP_STARTING_SESSION_CLASS,
  TEAM_MEMBER_ACP_TECHNICAL_SUMMARY_CLASS,
  TEAM_MEMBER_ACP_TITLE_SHELL_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberAcpPanelProps = {
  developerMode: boolean;
  selectedMemberId: string;
  memberTitle?: string | null;
  hideMemberTitle?: boolean;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedMemberRole?: string | null;
  selectedAgentStatus?: string | null;
  selectedTargetNodeId?: string | null;
  selectedSessionId?: string | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
  canControlAcp?: boolean;
  canInterrupt?: boolean;
  onInterrupt?: () => Promise<void> | void;
  onAcpSetMode?: (modeId: string) => Promise<void> | void;
  onAcpSetModel?: (modelId: string) => Promise<void> | void;
  onAcpSetConfig?: (configId: string, value: string) => Promise<void> | void;
  onForceNewSession?: () => Promise<void> | void;
  onLoadOlder?: () => Promise<void> | void;
};

const NOOP = () => {};

function TeamMemberAcpPanelImpl(props: TeamMemberAcpPanelProps) {
  const {
    selectedMemberId,
    developerMode,
    selectedMemberSnapshot,
    memberTitle: memberTitleProp,
    hideMemberTitle = false,
    selectedMemberRole,
    selectedAgentStatus,
    selectedTargetNodeId,
    selectedSessionId: selectedSessionIdProp,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    oldestMemberEventId,
    onSendInput,
    canControlAcp = false,
    canInterrupt,
    onInterrupt,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onForceNewSession,
    onLoadOlder = NOOP,
  } = props;

  const [acpTab, setAcpTab] = React.useState<TeamMemberAcpTab>("conversation");
  const [, setThinkingTick] = React.useState(0);
  const ansi = React.useCallback((inputValue: string) => inputValue, []);
  const normalizedAgentStatus = normalizeTeamMemberStatusValue(selectedAgentStatus);
  const normalizedSnapshotStatus =
    normalizeTeamMemberStatusValue(selectedMemberSnapshot?.status) ||
    normalizeTeamMemberStatusValue(selectedMemberSnapshot?.session_status);
  const snapshotHasActiveSession = isActiveTeamMemberStatus(normalizedSnapshotStatus);
  const explicitSelectedSessionId = selectedSessionIdProp?.trim() || null;
  const resolvedSelectedSessionId =
    normalizedSnapshotStatus && !snapshotHasActiveSession
      ? null
      : explicitSelectedSessionId ??
        selectedMemberSnapshot?.session_id?.trim() ??
        (normalizedAgentStatus &&
        !snapshotHasActiveSession &&
        !isAgentActiveStatus(normalizedAgentStatus)
          ? null
          : getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step));
  const {
    terminalRef,
    terminalShowJump,
    inputDockHeight,
    acpModeId,
    acpModelId,
    acpConfigId,
    acpConfigValue,
    setInputDockHeight,
    setAcpModeId,
    setAcpModelId,
    setAcpConfigId,
    setAcpConfigValue,
    handleTerminalScroll,
    jumpToTerminalBottom,
    resetInputDockHeight,
  } = useTeamMemberAcpPanelState();
  const {
    isComposingRef,
    input,
    inputHistory,
    sendingInput,
    canSendInput,
    handleSendInput,
    handleSubmitRequestUserInput,
    handleInputChange,
    handleNavigateHistory,
    handleSelectHistoryCommand,
  } = useTeamMemberAcpInput({
    selectedMemberId,
    selectedSessionId: resolvedSelectedSessionId,
    onSendInput,
  });
  const {
    selectedSessionId,
    effectiveAcpTab,
    acpView,
    terminalOutputs,
    hasVisibleInputDock,
    canInterruptAcpRun,
    memberTitle,
    memberModelLabel,
    memberStatus,
    memberStatusLabel,
    memberStatusClassToken,
    isStartingAcpSession,
    panelSubtitle,
    activitySummaryItems,
    developerTechnicalMetadata,
    acpPanelProps,
    inputDockJumpMode,
  } = useTeamMemberAcpViewModel({
    developerMode,
    selectedMemberId,
    memberTitle: memberTitleProp,
    selectedMemberSnapshot,
    selectedMemberRole,
    selectedAgentStatus,
    selectedSessionId: resolvedSelectedSessionId,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    oldestMemberEventId,
    acpTab,
    inputDockHeight,
    terminalShowJump,
    acpModeId,
    acpModelId,
    acpConfigId,
    acpConfigValue,
    canControlAcp,
    canInterrupt,
    onSendInput,
    onInterrupt,
    onAcpSetMode,
    onAcpSetModel,
    onAcpSetConfig,
    onForceNewSession,
    onLoadOlder,
    setAcpTab,
    setAcpModeId,
    setAcpModelId,
    setAcpConfigId,
    setAcpConfigValue,
    ansi,
    terminalRef,
    handleTerminalScroll,
    jumpToTerminalBottom,
    handleSubmitRequestUserInput,
  });
  React.useEffect(() => {
    setAcpTab("conversation");
  }, [selectedMemberId, selectedSessionId]);
  React.useEffect(() => {
    if (!acpView.thinkingStartTs) {
      return;
    }
    setThinkingTick(0);
    const timer = window.setInterval(() => {
      setThinkingTick((prev) => prev + 1);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [acpView.thinkingStartTs]);
  React.useEffect(() => {
    if (effectiveAcpTab !== "debug") {
      return;
    }
    handleTerminalScroll();
  }, [effectiveAcpTab, handleTerminalScroll, terminalOutputs.length]);
  React.useEffect(() => {
    if (!hasVisibleInputDock) {
      resetInputDockHeight();
    }
  }, [hasVisibleInputDock, resetInputDockHeight]);
  const shouldRenderPanel = Boolean(selectedMemberId.trim());
  const memberDescription = selectedMemberSnapshot?.description?.trim() || null;
  const attachedNodeId = selectedTargetNodeId?.trim() || null;
  const infoStripItems = [
    selectedMemberRole?.trim() || selectedMemberSnapshot?.role?.trim() || null,
    memberModelLabel,
    panelSubtitle,
  ].filter((value): value is string => Boolean(value));
  const infoStripSummary = infoStripItems.join(" · ");
  const acpPanelPropsWithoutSubtitle = React.useMemo(
    () => ({
      ...acpPanelProps,
      subtitle: null,
      headerContext: (
        <div className={TEAM_MEMBER_ACP_HEADER_CONTEXT_CLASS}>
          <StatusBadge
            label={memberStatusLabel}
            tone={resolveTeamRunStatusTone(memberStatus)}
            className={`agent-status status-${memberStatusClassToken}`}
            title={`status: ${memberStatusLabel}`}
          />
          {attachedNodeId ? (
            <a
              href={buildWorkspaceNodePath(attachedNodeId)}
              className={TEAM_MEMBER_ACP_NODE_LINK_CLASS}
              title={`Open node detail for ${attachedNodeId}`}
              onClick={(event) => {
                if (!shouldHandleInAppLinkClick(event)) {
                  return;
                }
                event.preventDefault();
                navigateToPath(buildWorkspaceNodePath(attachedNodeId));
              }}
            >
              <span className="truncate">{`Machine ${attachedNodeId}`}</span>
            </a>
          ) : null}
          {infoStripSummary ? (
            <div className={TEAM_MEMBER_ACP_INFO_STRIP_CLASS}>
              <Badge
                tone="outline"
                shape="pill"
                className={TEAM_MEMBER_ACP_INFO_BADGE_CLASS}
                title={infoStripSummary}
              >
                {infoStripSummary}
              </Badge>
            </div>
          ) : null}
          {activitySummaryItems.length > 0 ? (
            <div
              className={TEAM_MEMBER_ACP_ACTIVITY_STRIP_CLASS}
              data-team-member-acp-activity-strip="true"
            >
              {activitySummaryItems.map((item) => (
                <Badge
                  key={item.id}
                  tone="outline"
                  shape="pill"
                  className={TEAM_MEMBER_ACP_ACTIVITY_BADGE_CLASS}
                  title={item.title ?? item.label}
                >
                  {item.label}
                </Badge>
              ))}
            </div>
          ) : null}
          {isStartingAcpSession ? (
            <div className={TEAM_MEMBER_ACP_STARTING_SESSION_CLASS}>
              <span className={TEAM_MEMBER_ACP_STARTING_LABEL_CLASS}>Starting session</span>
              <span
                role="progressbar"
                aria-label="Starting ACP session"
                aria-valuetext="Starting ACP session"
                className={TEAM_MEMBER_ACP_STARTING_PROGRESS_TRACK_CLASS}
              >
                <span className={TEAM_MEMBER_ACP_STARTING_PROGRESS_BAR_CLASS} />
              </span>
            </div>
          ) : null}
          <OutputHeaderDetails
            items={developerTechnicalMetadata}
            summaryClassName={TEAM_MEMBER_ACP_TECHNICAL_SUMMARY_CLASS}
          />
        </div>
      ),
    }),
    [
      acpPanelProps,
      attachedNodeId,
      developerTechnicalMetadata,
      infoStripSummary,
      activitySummaryItems,
      isStartingAcpSession,
      memberStatus,
      memberStatusClassToken,
      memberStatusLabel,
    ]
  );

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} ${TEAM_MEMBER_ACP_PANEL_SHELL_CLASS}`}>
      {!selectedMemberId.trim() && (
        <p className={`mt-1 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {shouldRenderPanel && (
        <div className={TEAM_MEMBER_ACP_BODY_SHELL_CLASS}>
          {!hideMemberTitle && (
            <div className={TEAM_MEMBER_ACP_TITLE_SHELL_CLASS}>
              <div className={OUTPUT_HEADER_TITLE_CLASS}>
                <div className={OUTPUT_HEADER_TITLE_TEXT_CLASS}>
                  <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
                    <h2 className={OUTPUT_HEADER_TITLE_HEADING_CLASS}>{memberTitle}</h2>
                  </div>
                  {memberDescription ? (
                    <p className={TEAM_MEMBER_ACP_DESCRIPTION_CLASS}>
                      {memberDescription}
                    </p>
                  ) : null}
                </div>
              </div>
            </div>
          )}
          <div className={TEAM_MEMBER_ACP_PANEL_BODY_CLASS}>
            <AcpPanel {...acpPanelPropsWithoutSubtitle} />
          </div>
        </div>
      )}

      {hasVisibleInputDock && (
        <div className={TEAM_MEMBER_ACP_INPUT_DOCK_SHELL_CLASS}>
          <InputDock
            input={input}
            historyCommands={inputHistory}
            showInterrupt={Boolean(onInterrupt) && acpView.hasAcp}
            canInterrupt={canInterruptAcpRun}
            sendDisabled={!canSendInput || sendingInput}
            onHeightChange={setInputDockHeight}
            onInputChange={handleInputChange}
            onSendInput={() => {
              void handleSendInput();
            }}
            onInterrupt={() => {
              void onInterrupt?.();
            }}
            onNavigateHistory={handleNavigateHistory}
            onSelectHistoryCommand={handleSelectHistoryCommand}
            onJumpToBottom={inputDockJumpMode.onJumpToBottom}
            showConversationJump={inputDockJumpMode.showConversationJump}
            isComposingRef={isComposingRef}
          />
          {sendingInput && (
            <p className={`${TEAM_MEMBER_ACP_SENDING_TEXT_CLASS} ${TEAM_MUTED_TEXT_CLASS}`}>
              Sending prompt to selected agent...
            </p>
          )}
        </div>
      )}
    </div>
  );
}

export const TeamMemberAcpPanel = React.memo(TeamMemberAcpPanelImpl);
TeamMemberAcpPanel.displayName = "TeamMemberAcpPanel";
