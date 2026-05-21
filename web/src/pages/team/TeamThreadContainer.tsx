import React, { useCallback, useMemo } from "react";
import { TeamThreadPane } from "./team_thread_pane";
import {
  HUMAN_MAILBOX_ACTOR_ID,
  formatTs,
  resolveChatMessageText,
  resolveThreadRootMessageIdFromPayload,
} from "./page_helpers";
import { buildTeamWorkspacePath } from "../team_page";
import {
  createDisplayNameLookup,
  isHumanMailboxActor,
  normalizeRawMentionActorId,
  resolveDisplayName,
  type MentionCandidate,
} from "./mailbox_helpers";
import {
  useTeamConversationContext,
  useTeamWorkspaceShell,
} from "./team_workspace_context";

function collectThreadMentionActorIds(text: string): string[] {
  const actorIds = new Set<string>();
  for (const match of text.matchAll(/<at>\s*([A-Za-z0-9._:-]+)\s*<\/at>/gi)) {
    const actorId = match[1]?.trim();
    if (actorId) {
      actorIds.add(actorId);
    }
  }
  for (const match of text.matchAll(/(^|[\s([{'"])@([A-Za-z0-9._:-]+)/g)) {
    const actorId = normalizeRawMentionActorId(match[2] ?? "");
    if (actorId) {
      actorIds.add(actorId);
    }
  }
  return [...actorIds];
}

export const TeamThreadContainer = React.memo(function TeamThreadContainer() {
  const {
    taskMessages,
    threadReplyDraft,
    setThreadReplyDraft,
    onSendThreadReply,
    setChannelFocusMessageId,
  } = useTeamConversationContext();
  const {
    selectedChannelItem,
    routeThreadRootMessageId,
    busy,
    mailboxDisplayNameByActorId,
    selectedTeamMemberLiveStates,
    selectedConversationMatchesChannelLane,
    effectiveSelectedTeamId,
    routeWorkspaceLens,
    routeChannelId,
    activeChannelConversationTaskId,
    navigateTeamRoute,
  } = useTeamWorkspaceShell();
  const channelLabel = selectedChannelItem?.label ?? "";
  const replyBusy = busy === "send-thread-reply";
  const liveStateByMemberId = useMemo(
    () => new Map(selectedTeamMemberLiveStates.map((member) => [member.member_id, member])),
    [selectedTeamMemberLiveStates]
  );
  const resolveThreadActorLabel = useCallback(
    (actorId: string | null | undefined): string | null => {
      const normalizedActorId = actorId?.trim();
      if (!normalizedActorId) {
        return null;
      }
      const exactDisplayName = resolveDisplayName(
        normalizedActorId,
        mailboxDisplayNameByActorId,
        ""
      );
      if (exactDisplayName) {
        return exactDisplayName;
      }
      if (isHumanMailboxActor(normalizedActorId, HUMAN_MAILBOX_ACTOR_ID)) {
        return resolveDisplayName(
          HUMAN_MAILBOX_ACTOR_ID,
          mailboxDisplayNameByActorId,
          "You"
        );
      }
      const agentName = liveStateByMemberId.get(normalizedActorId)?.agent_name?.trim();
      return agentName || normalizedActorId;
    },
    [liveStateByMemberId, mailboxDisplayNameByActorId]
  );
  const threadDisplayNameByActorId = useMemo(() => {
    const humanDisplayName = resolveDisplayName(
      HUMAN_MAILBOX_ACTOR_ID,
      mailboxDisplayNameByActorId,
      "You"
    );
    return createDisplayNameLookup([
      ...Object.entries(mailboxDisplayNameByActorId),
      ...selectedTeamMemberLiveStates.map(
        (member): [string, string] => [
          member.member_id,
          member.agent_name?.trim() || member.member_id,
        ]
      ),
      ...taskMessages
        .filter((message) => isHumanMailboxActor(message.from_actor_id, HUMAN_MAILBOX_ACTOR_ID))
        .map((message): [string, string] => [message.from_actor_id, humanDisplayName]),
      ...taskMessages.flatMap((message) =>
        collectThreadMentionActorIds(resolveChatMessageText(message.payload) ?? "").map(
          (actorId): [string, string] => [
            actorId,
            resolveThreadActorLabel(actorId) ?? actorId,
          ]
        )
      ),
    ]);
  }, [
    mailboxDisplayNameByActorId,
    resolveThreadActorLabel,
    selectedTeamMemberLiveStates,
    taskMessages,
  ]);
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
          authorLabel: resolveThreadActorLabel(msg.from_actor_id),
          createdAt: msg.created_at,
          text: resolveChatMessageText(msg.payload) ?? "",
        })),
    [resolveThreadActorLabel, routeThreadRootMessageId, taskMessages]
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
      rootAuthorLabel={resolveThreadActorLabel(activeThreadRootMessage?.from_actor_id)}
      rootCreatedAt={activeThreadRootMessage?.created_at ?? null}
      rootText={activeThreadRootMessage ? resolveChatMessageText(activeThreadRootMessage.payload) : null}
      replies={activeThreadReplies}
      replyDraft={threadReplyDraft}
      onReplyDraftChange={setThreadReplyDraft}
      onSendReply={onSendThreadReply}
      replyBusy={replyBusy}
      mentionCandidates={threadMentionCandidates}
      displayNameByActorId={threadDisplayNameByActorId}
      formatTs={formatTs}
      onViewInChannel={onViewInChannel}
      onClose={onClose}
    />
  );
});
