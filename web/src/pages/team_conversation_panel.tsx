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
  conversationKey?: string;
  developerMode: boolean;
  token?: string | null;
  tasksLoading?: boolean;
  onRefreshTasks?: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: (payload: { text: string; mentionActorIds: string[] }) => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  snapshotMailboxMessages?: TeamActorMessageRecord[];
  humanActorId?: string;
  displayNameByActorId?: Record<string, string>;
  memberLiveStates?: TeamMemberLiveState[];
  memberIds?: string[];
  conversationTitle?: string;
  isChannelConversation?: boolean;
  messagesLoading: boolean;
  busy: string | null;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
  onOpenThread?: (messageId: number) => void;
  activeThreadMessageId?: number | null;
  jumpToMessageId?: number | null;
  onJumpToMessageSettled?: () => void;
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
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden">
      <TeamTaskPanel
        conversationKey={props.conversationKey}
        developerMode={props.developerMode}
        token={props.token}
        messageDraft={props.messageDraft}
        onMessageDraftChange={props.onMessageDraftChange}
        onSendMessage={props.onSendMessage}
        messages={messages}
        seenByMessageId={seenByMessageId}
        humanActorId={humanActorId}
        displayNameByActorId={props.displayNameByActorId}
        memberLiveStates={props.memberLiveStates}
        memberIds={memberIds}
        conversationTitle={props.conversationTitle}
        isChannelConversation={props.isChannelConversation}
        messagesLoading={props.messagesLoading}
        busy={props.busy}
        formatTs={props.formatTs}
        toPrettyJson={props.toPrettyJson}
        onOpenThread={props.onOpenThread}
        activeThreadMessageId={props.activeThreadMessageId}
        jumpToMessageId={props.jumpToMessageId}
        onJumpToMessageSettled={props.onJumpToMessageSettled}
      />
    </div>
  );
}

export const TeamConversationPanel = React.memo(TeamConversationPanelImpl);
TeamConversationPanel.displayName = "TeamConversationPanel";
