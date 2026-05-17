import React, { useCallback } from "react";
import { TeamConversationPanel } from "../team_conversation_panel";
import {
  HUMAN_MAILBOX_ACTOR_ID,
  formatTs,
  toPrettyJson,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../team_page";
import type {
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
  TeamRunSnapshotRecord,
} from "../../api";
import type { TeamMemberLiveState } from "./member_helpers";
import type { WorkspaceLens } from "../../app_route_selection";

type TeamConversationContainerProps = {
  selectedConversation: { id?: string | null } | null;
  developerMode: boolean;
  token: string;
  tasksLoading: boolean;
  onRefreshTasks: () => void;
  taskMessageDraft: string;
  setTaskMessageDraft: (val: string) => void;
  onSendTaskMessage: (payload: { text: string; mentionActorIds: string[] }) => void | Promise<void>;
  taskMessages: TeamConversationMessageRecord[];
  conversationMailboxMessages: TeamActorMessageRecord[];
  snapshot: TeamRunSnapshotRecord | null;
  mailboxDisplayNameByActorId: Record<string, string>;
  selectedTeamMemberLiveStates: TeamMemberLiveState[];
  taskConversationMemberIds: string[];
  activeConversationTitle: string;
  selectedConversationMatchesChannelLane: boolean;
  taskMessagesLoading: boolean;
  busy: string | null;
  routeThreadRootMessageId: number | null;
  channelFocusMessageId: number | null;
  setChannelFocusMessageId: (id: number | null) => void;
  effectiveSelectedTeamId: string | null;
  routeWorkspaceLens: WorkspaceLens | null;
  routeChannelId: string;
  activeChannelConversationTaskId: string | null;
  navigateTeamRoute: (path: string) => void;
};

export const TeamConversationContainer = React.memo(function TeamConversationContainer(
  props: TeamConversationContainerProps
) {
  const {
    selectedConversation,
    developerMode,
    token,
    tasksLoading,
    onRefreshTasks,
    taskMessageDraft,
    setTaskMessageDraft,
    onSendTaskMessage,
    taskMessages,
    conversationMailboxMessages,
    snapshot,
    mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates,
    taskConversationMemberIds,
    activeConversationTitle,
    selectedConversationMatchesChannelLane,
    taskMessagesLoading,
    busy,
    routeThreadRootMessageId,
    channelFocusMessageId,
    setChannelFocusMessageId,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
  } = props;

  const onOpenThread = useCallback(
    (messageId: number) => {
      if (effectiveSelectedTeamId && selectedConversationMatchesChannelLane) {
        navigateTeamRoute(
          buildTeamWorkspacePath(
            effectiveSelectedTeamId,
            routeWorkspaceLens,
            routeChannelId,
            messageId,
            null,
            null,
            activeChannelConversationTaskId
          )
        );
      }
    },
    [
      effectiveSelectedTeamId,
      selectedConversationMatchesChannelLane,
      navigateTeamRoute,
      routeWorkspaceLens,
      routeChannelId,
      activeChannelConversationTaskId,
    ]
  );

  return (
    <TeamConversationPanel
      conversationKey={selectedConversation?.id ?? undefined}
      developerMode={developerMode}
      token={token}
      tasksLoading={tasksLoading}
      onRefreshTasks={onRefreshTasks}
      messageDraft={taskMessageDraft}
      onMessageDraftChange={setTaskMessageDraft}
      onSendMessage={onSendTaskMessage}
      messages={taskMessages}
      conversationMailboxMessages={conversationMailboxMessages}
      snapshotMailboxMessages={snapshot?.mailbox.recent_messages ?? []}
      humanActorId={HUMAN_MAILBOX_ACTOR_ID}
      displayNameByActorId={mailboxDisplayNameByActorId}
      memberLiveStates={selectedTeamMemberLiveStates}
      memberIds={taskConversationMemberIds}
      conversationTitle={activeConversationTitle}
      isChannelConversation={selectedConversationMatchesChannelLane}
      messagesLoading={taskMessagesLoading}
      busy={busy}
      formatTs={formatTs}
      toPrettyJson={toPrettyJson}
      activeThreadMessageId={selectedConversationMatchesChannelLane ? routeThreadRootMessageId : null}
      jumpToMessageId={channelFocusMessageId}
      onJumpToMessageSettled={() => {
        setChannelFocusMessageId(null);
      }}
      onOpenThread={onOpenThread}
    />
  );
});
