import React, { useCallback } from "react";
import { TeamConversationPanel } from "../team_conversation_panel";
import {
  HUMAN_MAILBOX_ACTOR_ID,
  formatTs,
  toPrettyJson,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../../app_route_selection";
import {
  useTeamConversationContext,
  useTeamTasksContext,
  useTeamWorkspaceShell,
} from "./team_workspace_context";

export const TeamConversationContainer = React.memo(function TeamConversationContainer() {
  const {
    selectedConversation,
    token,
    taskMessageDraft,
    setTaskMessageDraft,
    onSendTaskMessage,
    taskMessages,
    conversationMailboxMessages,
    taskConversationMemberIds,
    activeConversationTitle,
    taskMessagesLoading,
    channelFocusMessageId,
    setChannelFocusMessageId,
  } = useTeamConversationContext();
  const { tasksLoading, onRefreshTasks } = useTeamTasksContext();
  const {
    developerMode,
    snapshot,
    mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates,
    selectedConversationMatchesChannelLane,
    busy,
    routeThreadRootMessageId,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
  } = useTeamWorkspaceShell();

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

  const onOpenMemberProfile = useCallback(
    (memberId: string) => {
      const normalizedMemberId = memberId.trim();
      if (!effectiveSelectedTeamId || !normalizedMemberId) {
        return;
      }
      navigateTeamRoute(
        buildTeamWorkspacePath(
          effectiveSelectedTeamId,
          "channels",
          routeChannelId,
          null,
          normalizedMemberId,
          null,
          activeChannelConversationTaskId
        )
      );
    },
    [activeChannelConversationTaskId, effectiveSelectedTeamId, navigateTeamRoute, routeChannelId]
  );

  return (
    <TeamConversationPanel
      conversationKey={selectedConversation?.id ?? undefined}
      developerMode={developerMode}
      token={token}
      selectedTeamId={effectiveSelectedTeamId}
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
      onOpenMemberProfile={onOpenMemberProfile}
      jumpToMessageId={channelFocusMessageId}
      onJumpToMessageSettled={() => {
        setChannelFocusMessageId(null);
      }}
      onOpenThread={onOpenThread}
    />
  );
});
