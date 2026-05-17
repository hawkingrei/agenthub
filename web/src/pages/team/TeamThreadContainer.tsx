import React, { useCallback, useMemo } from "react";
import { TeamThreadPane } from "./team_thread_pane";
import {
  formatTs,
  resolveChatMessageText,
  resolveThreadRootMessageIdFromPayload,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../team_page";
import type { MentionCandidate } from "./mailbox_helpers";
import { useTeamWorkspace } from "./team_workspace_context";

export const TeamThreadContainer = React.memo(function TeamThreadContainer() {
  const {
    selectedChannelItem,
    routeThreadRootMessageId,
    taskMessages,
    threadReplyDraft,
    setThreadReplyDraft,
    onSendThreadReply,
    busy,
    mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates,
    selectedConversationMatchesChannelLane,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
    setChannelFocusMessageId,
  } = useTeamWorkspace();
  const channelLabel = selectedChannelItem?.label ?? "";
  const replyBusy = busy === "send-thread-reply";
  const threadMentionCandidates = useMemo<MentionCandidate[]>(
    () =>
      selectedTeamMemberLiveStates.map((member) => {
        const actorId = member.member_id.trim();
        const label =
          member.agent_name?.trim() ||
          mailboxDisplayNameByActorId[actorId]?.trim() ||
          actorId;
        return {
          actorId,
          label,
          aliases: [actorId],
        };
      }),
    [mailboxDisplayNameByActorId, selectedTeamMemberLiveStates]
  );

  const activeThreadRootMessage = useMemo(
    () =>
      taskMessages.find((msg) => msg.message_id === routeThreadRootMessageId) ?? null,
    [routeThreadRootMessageId, taskMessages]
  );

  const activeThreadReplies = useMemo(
    () =>
      taskMessages
        .filter(
          (msg) =>
            msg.route === "team_thread_reply" &&
            resolveThreadRootMessageIdFromPayload(msg.payload) === routeThreadRootMessageId
        )
        .map((msg) => ({
          messageId: msg.message_id,
          authorLabel: msg.from_actor_id,
          createdAt: msg.created_at,
          text: resolveChatMessageText(msg.payload) ?? "",
        })),
    [routeThreadRootMessageId, taskMessages]
  );

  const buildCurrentThreadlessPath = useCallback(() => {
    if (!effectiveSelectedTeamId) {
      return null;
    }
    return buildTeamWorkspacePath(
      effectiveSelectedTeamId,
      routeWorkspaceLens,
      routeChannelId,
      null,
      null,
      null,
      activeChannelConversationTaskId
    );
  }, [
    activeChannelConversationTaskId,
    effectiveSelectedTeamId,
    routeChannelId,
    routeWorkspaceLens,
  ]);

  const onViewInChannel = useCallback(() => {
    setChannelFocusMessageId(routeThreadRootMessageId);
    const nextPath = buildCurrentThreadlessPath();
    if (nextPath) {
      navigateTeamRoute(nextPath);
    }
  }, [buildCurrentThreadlessPath, navigateTeamRoute, routeThreadRootMessageId, setChannelFocusMessageId]);

  const onClose = useCallback(() => {
    const nextPath = buildCurrentThreadlessPath();
    if (nextPath) {
      navigateTeamRoute(nextPath);
    }
  }, [buildCurrentThreadlessPath, navigateTeamRoute]);

  if (!selectedConversationMatchesChannelLane || !routeThreadRootMessageId) {
    return null;
  }

  return (
    <TeamThreadPane
      channelLabel={channelLabel}
      rootMessageId={activeThreadRootMessage?.message_id ?? routeThreadRootMessageId}
      rootAuthorLabel={activeThreadRootMessage?.from_actor_id ?? null}
      rootCreatedAt={activeThreadRootMessage?.created_at ?? null}
      rootText={activeThreadRootMessage ? resolveChatMessageText(activeThreadRootMessage.payload) : null}
      replies={activeThreadReplies}
      replyDraft={threadReplyDraft}
      onReplyDraftChange={setThreadReplyDraft}
      onSendReply={onSendThreadReply}
      replyBusy={replyBusy}
      mentionCandidates={threadMentionCandidates}
      formatTs={formatTs}
      onViewInChannel={onViewInChannel}
      onClose={onClose}
    />
  );
});
