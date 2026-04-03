import React from "react";
import type {
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
} from "../api";
import { TeamTaskPanel } from "./team_task_panel";
import { mergeMailboxMessages } from "./team/mailbox_helpers";
import { type TeamMemberLiveState } from "./team/member_helpers";
import { resolveTaskMessageSeenByActors } from "./team/page_helpers";

type TeamConversationPanelProps = {
  developerMode: boolean;
  token?: string | null;
  tasksLoading?: boolean;
  onRefreshTasks?: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: (payload: { text: string; mentionActorIds: string[] }) => Promise<void> | void;
  onRefreshMessages?: () => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  snapshotMailboxMessages?: TeamActorMessageRecord[];
  humanActorId?: string;
  memberLiveStates?: TeamMemberLiveState[];
  memberIds?: string[];
  messagesLoading: boolean;
  busy: string | null;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

function TeamConversationPanelImpl(props: TeamConversationPanelProps) {
  const {
    conversationMailboxMessages,
    humanActorId,
    memberIds = [],
    messages,
    snapshotMailboxMessages = [],
  } = props;

  const seenByMessageId = React.useMemo(
    () =>
      resolveTaskMessageSeenByActors(
        mergeMailboxMessages(snapshotMailboxMessages, conversationMailboxMessages),
        messages[0]?.conversation_id ?? "",
        memberIds
      ),
    [conversationMailboxMessages, memberIds, messages, snapshotMailboxMessages]
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <TeamTaskPanel
        developerMode={props.developerMode}
        token={props.token}
        messageDraft={props.messageDraft}
        onMessageDraftChange={props.onMessageDraftChange}
        onSendMessage={props.onSendMessage}
        messages={messages}
        seenByMessageId={seenByMessageId}
        humanActorId={humanActorId}
        memberLiveStates={props.memberLiveStates}
        memberIds={memberIds}
        messagesLoading={props.messagesLoading}
        busy={props.busy}
        formatTs={props.formatTs}
        toPrettyJson={props.toPrettyJson}
      />
    </div>
  );
}

export const TeamConversationPanel = React.memo(TeamConversationPanelImpl);
TeamConversationPanel.displayName = "TeamConversationPanel";
