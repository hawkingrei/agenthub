import React, { useCallback } from "react";
import { TeamConversationPanel } from "../team_conversation_panel";
import {
  HUMAN_MAILBOX_ACTOR_ID,
  formatTs,
  toPrettyJson,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../team_page";
import { useTeamWorkspace } from "./team_workspace_context";

export const TeamConversationContainer = React.memo(function TeamConversationContainer() {
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
  } = useTeamWorkspace();

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
