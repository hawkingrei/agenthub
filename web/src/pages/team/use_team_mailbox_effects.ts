import { useEffect } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { AgentEvent, TeamActorMessageRecord, TeamRunSnapshotRecord } from "../../api";
import type { TeamTab } from "./state";

type UseTeamMailboxEffectsParams = {
  snapshot: TeamRunSnapshotRecord | null;
  selectedMemberId: string;
  activeRunId: string | null;
  chatInboxActorId: string;
  tab: TeamTab;
  chatStickToBottom: boolean;
  conversationKey: string;
  conversationLatestMessageId: number | null;
  conversationMessageCount: number;
  loadInbox: (actorIdOverride?: string) => Promise<unknown>;
  loadMemberEvents: (mode?: "replace" | "prepend") => Promise<unknown>;
  markConversationSeen: (key: string, messageId: number | null) => void;
  scrollConversationToBottom: () => void;
  parseErrorMessage: (error: unknown) => string;
  setError: (value: string | null) => void;
  setSelectedMemberId: (next: string) => void;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setInbox: (next: TeamActorMessageRecord[]) => void;
  setInboxActorId: (next: string) => void;
  setChatStickToBottom: (next: boolean) => void;
};

export function useTeamMailboxEffects({
  snapshot,
  selectedMemberId,
  activeRunId,
  chatInboxActorId,
  tab,
  chatStickToBottom,
  conversationKey,
  conversationLatestMessageId,
  conversationMessageCount,
  loadInbox,
  loadMemberEvents,
  markConversationSeen,
  scrollConversationToBottom,
  parseErrorMessage,
  setError,
  setSelectedMemberId,
  setMemberEvents,
  setInbox,
  setInboxActorId,
  setChatStickToBottom,
}: UseTeamMailboxEffectsParams) {
  useEffect(() => {
    if (!snapshot) {
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    if (
      selectedMemberId &&
      snapshot.members.some((member) => member.member_id === selectedMemberId)
    ) {
      return;
    }
    setSelectedMemberId(snapshot.members[0]?.member_id ?? "");
  }, [selectedMemberId, setMemberEvents, setSelectedMemberId, snapshot]);

  useEffect(() => {
    const actorId = chatInboxActorId.trim();
    if (!activeRunId || !actorId) {
      setInbox([]);
      return;
    }
    setInboxActorId(actorId);
    void loadInbox(actorId).catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [
    activeRunId,
    chatInboxActorId,
    loadInbox,
    parseErrorMessage,
    setError,
    setInbox,
    setInboxActorId,
  ]);

  useEffect(() => {
    if (tab !== "mailbox") {
      return;
    }
    setChatStickToBottom(true);
    window.requestAnimationFrame(() => {
      scrollConversationToBottom();
    });
  }, [conversationKey, scrollConversationToBottom, setChatStickToBottom, tab]);

  useEffect(() => {
    if (tab !== "mailbox" || !chatStickToBottom) {
      return;
    }
    window.requestAnimationFrame(() => {
      scrollConversationToBottom();
      markConversationSeen(conversationKey, conversationLatestMessageId);
    });
  }, [
    chatStickToBottom,
    conversationKey,
    conversationLatestMessageId,
    conversationMessageCount,
    markConversationSeen,
    scrollConversationToBottom,
    tab,
  ]);

  useEffect(() => {
    void loadMemberEvents("replace").catch((err) => {
      setError(parseErrorMessage(err));
    });
  }, [loadMemberEvents, parseErrorMessage, setError]);
}
