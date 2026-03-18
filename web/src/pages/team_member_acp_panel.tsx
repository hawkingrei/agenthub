import React from "react";
import { buildAcpView } from "../acp";
import { AgentEvent, TeamMemberSnapshot, getTeamStepRuntimeHandleId } from "../api";
import { AcpConversation } from "../components/acp_conversation";
import { InputDock } from "../components/input_dock";
import { useAcpConversation } from "../hooks/use_acp_conversation";
import { pushInputHistory } from "../input_history";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberAcpPanelProps = {
  developerMode: boolean;
  selectedMemberId: string;
  selectedMemberSnapshot: TeamMemberSnapshot | null;
  selectedMemberRole?: string | null;
  selectedSessionId?: string | null;
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  onSendInput?: (input: string, sessionId: string) => Promise<void> | void;
  onRefresh: () => Promise<void> | void;
  onLoadOlder: () => Promise<void> | void;
};

export function TeamMemberAcpPanel(props: TeamMemberAcpPanelProps) {
  const {
    selectedMemberId,
    developerMode,
    selectedMemberSnapshot,
    selectedMemberRole,
    selectedSessionId: selectedSessionIdProp,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    eventsLoading,
    oldestMemberEventId,
    onSendInput,
    onRefresh,
    onLoadOlder,
  } = props;

  const selectedSessionId =
    selectedSessionIdProp ?? getTeamStepRuntimeHandleId(selectedMemberSnapshot?.latest_step);
  const acpEventLines = React.useMemo(
    () =>
      memberEvents.map((event) => ({
        ts: event.ts,
        seq: event.seq,
        event_id: event.event_id,
        stream: event.stream,
        message: event.message,
        session_id: event.session_id,
      })),
    [memberEvents]
  );
  const acpView = React.useMemo(() => buildAcpView(acpEventLines), [acpEventLines]);
  const conversationEventMeta = React.useMemo(() => {
    const memberId = selectedMemberId.trim();
    if (!memberId || !selectedSessionId) {
      return {};
    }
    return {
      [`${memberId}:${selectedSessionId}`]: {
        oldestId: oldestMemberEventId,
        hasMore: memberEventsHasMore,
        loading: memberEventsLoading,
        loaded: !memberEventsLoading || memberEvents.length > 0,
      },
    };
  }, [
    memberEvents.length,
    memberEventsHasMore,
    memberEventsLoading,
    oldestMemberEventId,
    selectedMemberId,
    selectedSessionId,
  ]);
  const acpConversation = useAcpConversation({
    acpView,
    activeAgent: selectedMemberId.trim() || null,
    activeSessionId: selectedSessionId ?? null,
    acpTab: "conversation",
    eventMeta: conversationEventMeta,
    isAgentActive: Boolean(selectedSessionId),
    onLoadOlder: () => {
      void onLoadOlder();
    },
  });
  const isComposingRef = React.useRef(false);
  const inputHistoryDraftRef = React.useRef("");
  const [threadOptionsOpen, setThreadOptionsOpen] = React.useState(false);
  const [input, setInput] = React.useState("");
  const [inputHistory, setInputHistory] = React.useState<string[]>([]);
  const [inputHistoryCursor, setInputHistoryCursor] = React.useState(-1);
  const [sendingInput, setSendingInput] = React.useState(false);
  const sendingInputRef = React.useRef(false);

  React.useEffect(() => {
    setThreadOptionsOpen(false);
  }, [selectedMemberId, selectedSessionId]);
  React.useEffect(() => {
    setInput("");
    setInputHistory([]);
    setInputHistoryCursor(-1);
    inputHistoryDraftRef.current = "";
  }, [selectedMemberId, selectedSessionId]);

  const canLoadOlder =
    Boolean(selectedMemberId.trim() && selectedSessionId) &&
    !memberEventsLoading &&
    memberEventsHasMore &&
    oldestMemberEventId != null;
  const shouldRenderConversation =
    Boolean(selectedMemberId.trim() && selectedSessionId) &&
    (memberEventsLoading ||
      acpView.hasAcp ||
      acpConversation.conversationTotalItems > 0);
  const showJumpButton = acpConversation.showConversationJump;
  const canSendInput = Boolean(selectedMemberId.trim() && selectedSessionId && onSendInput);
  const handleSendInput = React.useCallback(async () => {
    const text = input.trim();
    if (!text || !selectedSessionId || !onSendInput || sendingInputRef.current) {
      return;
    }
    sendingInputRef.current = true;
    setSendingInput(true);
    try {
      await onSendInput(text, selectedSessionId);
      setInputHistory((prev) => pushInputHistory(prev, text));
      setInputHistoryCursor(-1);
      inputHistoryDraftRef.current = "";
      setInput("");
    } finally {
      sendingInputRef.current = false;
      setSendingInput(false);
    }
  }, [input, onSendInput, selectedSessionId]);
  const handleInputChange = React.useCallback(
    (value: string) => {
      setInput(value);
      if (inputHistoryCursor >= 0) {
        setInputHistoryCursor(-1);
      }
      inputHistoryDraftRef.current = value;
    },
    [inputHistoryCursor]
  );
  const handleNavigateHistory = React.useCallback(
    (direction: "up" | "down") => {
      if (inputHistory.length === 0) {
        return;
      }
      if (direction === "up") {
        if (inputHistoryCursor < 0) {
          inputHistoryDraftRef.current = input;
          setInputHistoryCursor(0);
          setInput(inputHistory[0] ?? "");
          return;
        }
        const nextCursor = Math.min(inputHistory.length - 1, inputHistoryCursor + 1);
        setInputHistoryCursor(nextCursor);
        setInput(inputHistory[nextCursor] ?? "");
        return;
      }
      if (inputHistoryCursor < 0) {
        return;
      }
      if (inputHistoryCursor === 0) {
        setInputHistoryCursor(-1);
        setInput(inputHistoryDraftRef.current);
        return;
      }
      const nextCursor = inputHistoryCursor - 1;
      setInputHistoryCursor(nextCursor);
      setInput(inputHistory[nextCursor] ?? "");
    },
    [input, inputHistory, inputHistoryCursor]
  );
  const handleSelectHistoryCommand = React.useCallback(
    (value: string) => {
      const nextCursor = inputHistory.findIndex((item) => item === value);
      setInputHistoryCursor(nextCursor);
      setInput(value);
      inputHistoryDraftRef.current = value;
    },
    [inputHistory]
  );

  return (
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            type="button"
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={() => setThreadOptionsOpen((current) => !current)}
            aria-expanded={threadOptionsOpen}
            aria-label="Toggle thread options"
            title="Thread options"
          >
            <i className="bi bi-three-dots" aria-hidden="true" />
          </button>
        </div>
      </div>

      {threadOptionsOpen && (
        <div className="mt-3 flex flex-col gap-3 rounded-xl border border-ui-border bg-ui-surface-soft/60 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={() => {
                void onRefresh();
              }}
              disabled={selectedMemberId.trim() && selectedSessionId ? memberEventsLoading : eventsLoading}
              className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
              title="Refresh thread"
              aria-label="Refresh thread"
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              <span>Refresh Thread</span>
            </button>
            <button
              onClick={() => {
                void onLoadOlder();
              }}
              disabled={!canLoadOlder}
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            >
              Load Older
            </button>
          </div>
          {developerMode && (
            <div className="mono flex flex-wrap items-center gap-2 text-xs text-ui-text-muted">
              <div className="rounded-lg border border-ui-border bg-ui-surface px-3 py-2">
                member={selectedMemberId || "-"}
              </div>
              <div className="rounded-lg border border-ui-border bg-ui-surface px-3 py-2">
                role={selectedMemberSnapshot?.role ?? selectedMemberRole ?? "-"}
              </div>
              <div className="rounded-lg border border-ui-border bg-ui-surface px-3 py-2">
                session={selectedSessionId ?? "-"}
              </div>
            </div>
          )}
        </div>
      )}

      {!selectedMemberId.trim() && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {selectedMemberId.trim() && !selectedSessionId && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Selected agent has no thread session yet.
        </p>
      )}

      {selectedMemberId.trim() && selectedSessionId && !memberEventsLoading && !acpView.hasAcp && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          No thread events found in this agent session.
        </p>
      )}

      {shouldRenderConversation && (
        <div className="relative mt-3">
          <AcpConversation
            items={acpConversation.conversationRenderItems}
            windowOffset={acpConversation.conversationWindowOffset}
            isFrozenView={acpConversation.isFrozenView}
            shouldAutoCollapse={acpConversation.shouldAutoCollapse}
            collapseCutoff={acpConversation.collapseCutoff}
            runStatus={acpView.runStatus?.status ?? null}
            virtualTopSpacer={acpConversation.conversationVirtualTopSpacer}
            virtualBottomSpacer={acpConversation.conversationVirtualBottomSpacer}
            stickToBottom={acpConversation.conversationStickToBottom}
            pendingCount={acpConversation.conversationPendingCount}
            avgHeight={acpConversation.conversationAvgHeight}
            topHint={
              memberEventsLoading
                ? "Loading ACP events..."
                : acpConversation.showConversationTopReachedHint
                  ? "Already at top"
                  : null
            }
            focusedToolCallId={acpConversation.focusedConversationToolCallId}
            onScroll={acpConversation.handleConversationScroll}
            containerRef={acpConversation.acpConversationRef}
            ansi={(input) => input}
          />
          {showJumpButton && (
            <button
              className={ACP_JUMP_BOTTOM_BUTTON_CLASS}
              onClick={acpConversation.jumpToConversationBottom}
              title="Jump to bottom"
              aria-label="Jump to bottom"
            >
              <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
            </button>
          )}
        </div>
      )}

      {canSendInput && (
        <div className="mt-3">
          <InputDock
            input={input}
            historyCommands={inputHistory}
            showInterrupt={false}
            canInterrupt={false}
            sendDisabled={!canSendInput || sendingInput}
            onInputChange={handleInputChange}
            onSendInput={() => {
              void handleSendInput();
            }}
            onInterrupt={() => {}}
            onNavigateHistory={handleNavigateHistory}
            onSelectHistoryCommand={handleSelectHistoryCommand}
            onJumpToBottom={acpConversation.jumpToConversationBottom}
            showConversationJump={showJumpButton}
            isComposingRef={isComposingRef}
          />
          {sendingInput && (
            <p className={`mt-2 ${TEAM_MUTED_TEXT_CLASS}`}>Sending prompt to selected agent...</p>
          )}
        </div>
      )}
    </div>
  );
}
