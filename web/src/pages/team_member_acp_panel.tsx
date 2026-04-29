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
  const normalizedAgentStatus = selectedAgentStatus?.trim().toLowerCase() ?? "";
  const resolvedSelectedSessionId =
    normalizedAgentStatus && !isAgentActiveStatus(normalizedAgentStatus)
      ? null
      : selectedSessionIdProp ?? getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step);
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
    panelSubtitle,
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
        <div className="inline-flex min-w-0 max-w-full flex-wrap items-center gap-1 rounded-2xl border border-notion-border/80 bg-notion-sidebar/68 px-1.5 py-1 shadow-[0_1px_2px_rgba(15,23,42,0.04)]">
          <StatusBadge
            label={memberStatusLabel}
            tone={resolveTeamRunStatusTone(memberStatus)}
            className={`agent-status status-${memberStatusClassToken}`}
            title={`status: ${memberStatusLabel}`}
          />
          {attachedNodeId ? (
            <a
              href={buildWorkspaceNodePath(attachedNodeId)}
              className="inline-flex min-w-0 max-w-full items-center rounded-xl border border-transparent px-1.5 py-0.5 text-[11px] font-medium text-notion-text-muted transition hover:bg-white/70 hover:text-notion-text"
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
            <div className="min-w-0 max-w-full">
              <Badge
                tone="outline"
                shape="pill"
                className="max-w-full truncate border-transparent bg-transparent px-1.5 py-0.5 text-[11px] font-medium normal-case tracking-normal text-notion-text-muted shadow-none"
                title={infoStripSummary}
              >
                {infoStripSummary}
              </Badge>
            </div>
          ) : null}
          <OutputHeaderDetails
            items={developerTechnicalMetadata}
            summaryClassName="inline-flex cursor-pointer list-none items-center rounded-xl border border-transparent bg-transparent px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-[0.12em] text-notion-text-muted/70 transition hover:bg-white/70 hover:text-notion-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-notion-accent/10"
          />
        </div>
      ),
    }),
    [
      acpPanelProps,
      attachedNodeId,
      developerTechnicalMetadata,
      infoStripSummary,
      memberStatus,
      memberStatusClassToken,
      memberStatusLabel,
    ]
  );

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} px-2 pb-2 pt-1.5`}>
      {!selectedMemberId.trim() && (
        <p className={`mt-1 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {shouldRenderPanel && (
        <div className="relative mt-1 flex min-h-0 flex-1 flex-col gap-0 sm:gap-1">
          {!hideMemberTitle && (
            <div className="shrink-0 px-4 pt-0.5 sm:px-6">
              <div className={OUTPUT_HEADER_TITLE_CLASS}>
                <div className={OUTPUT_HEADER_TITLE_TEXT_CLASS}>
                  <div className={OUTPUT_HEADER_TITLE_MAIN_CLASS}>
                    <h2 className={OUTPUT_HEADER_TITLE_HEADING_CLASS}>{memberTitle}</h2>
                  </div>
                  {memberDescription ? (
                    <p className="mt-0.5 text-[12px] leading-5 text-notion-text-muted">
                      {memberDescription}
                    </p>
                  ) : null}
                </div>
              </div>
            </div>
          )}
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <AcpPanel {...acpPanelPropsWithoutSubtitle} />
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
