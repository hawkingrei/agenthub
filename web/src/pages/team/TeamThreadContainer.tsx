import React, { useCallback, useMemo } from "react";
import { TeamThreadPane } from "./team_thread_pane";
import {
  formatTs,
  resolveChatMessageText,
  resolveThreadRootMessageIdFromPayload,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../team_page";
import type { TeamConversationMessageRecord } from "../../api";
import type { MentionCandidate } from "./mailbox_helpers";
import type { WorkspaceLens } from "../../app_route_selection";

type TeamThreadContainerProps = {
  channelLabel: string;
  routeThreadRootMessageId: number | null;
  taskMessages: TeamConversationMessageRecord[];
  threadReplyDraft: string;
  setThreadReplyDraft: (val: string) => void;
  onSendThreadReply: (payload: { text: string; mentionActorIds: string[] }) => void;
  replyBusy: boolean;
  threadMentionCandidates: MentionCandidate[];
  effectiveSelectedTeamId: string | null;
  routeWorkspaceLens: WorkspaceLens | null;
  routeChannelId: string;
  activeChannelConversationTaskId: string | null;
  navigateTeamRoute: (path: string) => void;
  setChannelFocusMessageId: (id: number | null) => void;
};

export const TeamThreadContainer = React.memo(function TeamThreadContainer(
  props: TeamThreadContainerProps
) {
  const {
    channelLabel,
    routeThreadRootMessageId,
    taskMessages,
    threadReplyDraft,
    setThreadReplyDraft,
    onSendThreadReply,
    replyBusy,
    threadMentionCandidates,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
    setChannelFocusMessageId,
  } = props;

  const activeThreadRootMessage = useMemo(
    () =>
      taskMessages.find((msg) => msg.message_id === routeThreadRootMessageId) ?? null,
    [routeThreadRootMessageId, taskMessages]
  );

  const activeThreadReplies = useMemo(
    () =>
      taskMessages
        .filter((msg) => resolveThreadRootMessageIdFromPayload(msg.payload) === routeThreadRootMessageId)
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

  if (!routeThreadRootMessageId) {
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
