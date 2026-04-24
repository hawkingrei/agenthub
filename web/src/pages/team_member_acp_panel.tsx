import React from "react";
import { AgentEvent, TeamMemberSnapshot } from "../api";
import { getTeamStepRuntimeHandleId } from "../api";
import {
  AcpPanel,
} from "../components/acp_panel";
import { OutputHeaderDetails } from "../components/output_header";
import { InputDock } from "../components/input_dock";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TeamMemberAcpTab,
  useTeamMemberAcpViewModel,
} from "./team/use_team_member_acp_view_model";
import { useTeamMemberAcpInput } from "./team/use_team_member_acp_input";
import { useTeamMemberAcpPanelState } from "./team/use_team_member_acp_panel_state";
import {
  OUTPUT_HEADER_META_CLASS,
  OUTPUT_HEADER_ROOT_CLASS,
  OUTPUT_HEADER_TITLE_CLASS,
  OUTPUT_HEADER_TITLE_HEADING_CLASS,
  OUTPUT_HEADER_TITLE_MAIN_CLASS,
  OUTPUT_HEADER_TITLE_TEXT_CLASS,
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
  const resolvedSelectedSessionId =
    selectedSessionIdProp ?? getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step);
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
    developerTechnicalMetadata,
    acpPanelProps,
    inputDockJumpMode,
  } = useTeamMemberAcpViewModel({
    developerMode,
    selectedMemberId,
    memberTitle: memberTitleProp,
    selectedMemberSnapshot,
    selectedMemberRole,
    selectedSessionId: selectedSessionIdProp,
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

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-2`}>
      {!selectedMemberId.trim() && (
        <p className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {shouldRenderPanel && (
        <div className="relative mt-2 flex min-h-0 flex-1 flex-col gap-0 sm:gap-1.5">
          <div className={`${OUTPUT_HEADER_ROOT_CLASS} shrink-0`}>
            {!hideMemberTitle && (
              <div className={OUTPUT_HEADER_TITLE_CLASS}>
                <div className={OUTPUT_HEADER_TITLE_TEXT_CLASS}>
                  <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
                    <h2 className={OUTPUT_HEADER_TITLE_HEADING_CLASS}>{memberTitle}</h2>
                    {memberModelLabel ? (
                      <span className="agent-tag hidden sm:inline-flex">{memberModelLabel}</span>
                    ) : null}
                  </div>
                </div>
              </div>
            )}
            <div className={OUTPUT_HEADER_META_CLASS}>
              <StatusBadge
                label={memberStatusLabel}
                tone={resolveTeamRunStatusTone(memberStatus)}
                className={`agent-status status-${memberStatusClassToken}`}
                title={`status: ${memberStatusLabel}`}
              />
              {hideMemberTitle && memberModelLabel ? (
                <span className="agent-tag hidden sm:inline-flex">{memberModelLabel}</span>
              ) : null}
              <OutputHeaderDetails items={developerTechnicalMetadata} />
            </div>
          </div>
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <AcpPanel {...acpPanelProps} />
          </div>
        </div>
      )}

      {hasVisibleInputDock && (
        <div className="mt-1.5 shrink-0">
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
            <p className={`mt-1.5 ${TEAM_MUTED_TEXT_CLASS}`}>Sending prompt to selected agent...</p>
          )}
        </div>
      )}
    </div>
  );
}

export const TeamMemberAcpPanel = React.memo(TeamMemberAcpPanelImpl);
TeamMemberAcpPanel.displayName = "TeamMemberAcpPanel";
