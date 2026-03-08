import React from "react";
import { buildAcpView } from "../acp";
import { AgentEvent, TeamMemberSnapshot } from "../api";
import { AcpConversation } from "../components/acp_conversation";
import { buildConversationMessages } from "../conversation";
import { isNearBottom } from "../scroll";
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
  memberEvents: AgentEvent[];
  memberEventsHasMore: boolean;
  memberEventsLoading: boolean;
  eventsLoading: boolean;
  oldestMemberEventId: number | null;
  onRefresh: () => Promise<void> | void;
  onLoadOlder: () => Promise<void> | void;
};

export function TeamMemberAcpPanel(props: TeamMemberAcpPanelProps) {
  const {
    selectedMemberId,
    developerMode,
    selectedMemberSnapshot,
    memberEvents,
    memberEventsHasMore,
    memberEventsLoading,
    eventsLoading,
    oldestMemberEventId,
    onRefresh,
    onLoadOlder,
  } = props;

  const selectedSessionId = selectedMemberSnapshot?.latest_step?.remote_task_id ?? null;
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
  const conversationItems = React.useMemo(
    () =>
      buildConversationMessages(
        acpView.messages,
        acpView.toolCalls,
        acpView.plan,
        selectedSessionId
      ),
    [acpView.messages, acpView.plan, acpView.toolCalls, selectedSessionId]
  );

  const conversationRef = React.useRef<HTMLDivElement>(null);
  const [stickToBottom, setStickToBottom] = React.useState(true);
  const [threadOptionsOpen, setThreadOptionsOpen] = React.useState(false);

  React.useEffect(() => {
    setThreadOptionsOpen(false);
  }, [selectedMemberId, selectedSessionId]);

  React.useEffect(() => {
    if (!stickToBottom) {
      return;
    }
    const container = conversationRef.current;
    if (!container) {
      return;
    }
    container.scrollTop = container.scrollHeight;
  }, [conversationItems.length, stickToBottom]);

  const onConversationScroll = React.useCallback(() => {
    const container = conversationRef.current;
    if (!container) {
      return;
    }
    setStickToBottom(
      isNearBottom(
        container.scrollHeight,
        container.scrollTop,
        container.clientHeight
      )
    );
  }, []);

  const onJumpToBottom = React.useCallback(() => {
    const container = conversationRef.current;
    if (!container) {
      return;
    }
    container.scrollTop = container.scrollHeight;
    setStickToBottom(true);
  }, []);

  const canLoadOlder =
    Boolean(selectedMemberSnapshot) &&
    !memberEventsLoading &&
    memberEventsHasMore &&
    oldestMemberEventId != null;
  const shouldRenderConversation =
    Boolean(selectedMemberSnapshot && selectedSessionId) &&
    (memberEventsLoading || acpView.hasAcp || conversationItems.length > 0);
  const showJumpButton = !stickToBottom && conversationItems.length > 0;

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
              disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
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
                role={selectedMemberSnapshot?.role ?? "-"}
              </div>
              <div className="rounded-lg border border-ui-border bg-ui-surface px-3 py-2">
                session={selectedSessionId ?? "-"}
              </div>
            </div>
          )}
        </div>
      )}

      {!selectedMemberSnapshot && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select an agent from the left rail to inspect its thread.
        </p>
      )}

      {selectedMemberSnapshot && !selectedSessionId && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Selected agent has no thread session yet.
        </p>
      )}

      {selectedMemberSnapshot && selectedSessionId && !memberEventsLoading && !acpView.hasAcp && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          No thread events found in this agent session.
        </p>
      )}

      {shouldRenderConversation && (
        <div className="relative mt-3">
          <AcpConversation
            items={conversationItems}
            windowOffset={0}
            isFrozenView={false}
            shouldAutoCollapse={true}
            collapseCutoff={0}
            runStatus={acpView.runStatus?.status ?? null}
            virtualTopSpacer={0}
            virtualBottomSpacer={0}
            stickToBottom={stickToBottom}
            pendingCount={0}
            avgHeight={48}
            topHint={memberEventsLoading ? "Loading ACP events..." : null}
            focusedToolCallId={null}
            onScroll={onConversationScroll}
            containerRef={conversationRef}
            ansi={(input) => input}
          />
          {showJumpButton && (
            <button
              className={ACP_JUMP_BOTTOM_BUTTON_CLASS}
              onClick={onJumpToBottom}
              title="Jump to bottom"
              aria-label="Jump to bottom"
            >
              <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
