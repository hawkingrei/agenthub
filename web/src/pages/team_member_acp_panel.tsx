import React from "react";
import { buildAcpView } from "../acp";
import { AgentEvent, TeamMemberSnapshot, TeamRunSnapshotRecord } from "../api";
import { AcpConversation } from "../components/acp_conversation";
import { buildConversationMessages } from "../conversation";
import { isNearBottom } from "../scroll";
import {
  ACP_JUMP_BOTTOM_BUTTON_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

type TeamMemberAcpPanelProps = {
  snapshot: TeamRunSnapshotRecord | null;
  selectedMemberId: string;
  onSelectedMemberIdChange: (memberId: string) => void;
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
    snapshot,
    selectedMemberId,
    onSelectedMemberIdChange,
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
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Agent ACP</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            onClick={() => {
              void onRefresh();
            }}
            disabled={selectedMemberSnapshot ? memberEventsLoading : eventsLoading}
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            title="Refresh agent ACP"
            aria-label="Refresh agent ACP"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh</span>
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
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={selectedMemberId}
          onChange={(event) => onSelectedMemberIdChange(event.target.value)}
        >
          <option value="">Select member</option>
          {snapshot?.members.map((member) => (
            <option key={member.member_id} value={member.member_id}>
              {member.member_id} ({member.role})
            </option>
          ))}
        </select>
        <div className="mono rounded-lg border border-ui-border bg-ui-surface px-3 py-2 text-xs text-ui-text-muted">
          session={selectedSessionId ?? "-"}
        </div>
      </div>

      {!selectedMemberSnapshot && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Select a member to inspect ACP conversation events.
        </p>
      )}

      {selectedMemberSnapshot && !selectedSessionId && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          Selected member has no ACP session yet.
        </p>
      )}

      {selectedMemberSnapshot && selectedSessionId && !memberEventsLoading && !acpView.hasAcp && (
        <p className={`mt-3 ${TEAM_MUTED_TEXT_CLASS}`}>
          No ACP events found in this member session.
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
