import { useEffect, type Dispatch, type SetStateAction } from "react";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamRunSnapshotRecord,
} from "../../api";
import type { TeamTab } from "./state";

type UseTeamMailboxLifecycleEffectsOptions = {
  snapshot: TeamRunSnapshotRecord | null;
  selectedMemberId: string;
  mailboxActorIds?: string[];
  activeRunIdForSelectedTeam: string | null;
  chatInboxActorId: string;
  tab: TeamTab;
  chatStickToBottom: boolean;
  conversationKey: string;
  conversationLatestMessageId: number | null;
  conversationMessagesLength: number;
  loadInbox: (actorIdOverride?: string) => Promise<void>;
  loadMemberEvents: (
    mode?: "replace" | "prepend",
    sessionIdOverride?: string | null
  ) => Promise<void>;
  parseError: (err: unknown) => string;
  setError: Dispatch<SetStateAction<string | null>>;
  setSelectedMemberId: (next: string) => void;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setInbox: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
  setInboxActorId: (next: string) => void;
  setChatStickToBottom: (next: boolean) => void;
  scrollConversationToBottom: () => void;
  markConversationSeen: (key: string, messageId: number | null) => void;
};

export function useTeamMailboxLifecycleEffects(
  options: UseTeamMailboxLifecycleEffectsOptions
) {
  const {
    snapshot,
    selectedMemberId,
    mailboxActorIds = [],
    activeRunIdForSelectedTeam,
    chatInboxActorId,
    tab,
    chatStickToBottom,
    conversationKey,
    conversationLatestMessageId,
    conversationMessagesLength,
    loadInbox,
    loadMemberEvents,
    parseError,
    setError,
    setSelectedMemberId,
    setMemberEvents,
    setInbox,
    setInboxActorId,
    setChatStickToBottom,
    scrollConversationToBottom,
    markConversationSeen,
  } = options;

  useEffect(() => {
    if (tab !== "mailbox") {
      return;
    }
    if (!snapshot) {
      setSelectedMemberId("");
      setMemberEvents([]);
      return;
    }
    const selectableActorIds = mailboxActorIds.length > 0
      ? mailboxActorIds
      : snapshot.members.map((member) => member.member_id);
    if (
      selectedMemberId &&
      selectableActorIds.includes(selectedMemberId)
    ) {
      return;
    }
    setSelectedMemberId(selectableActorIds[0] ?? "");
  }, [mailboxActorIds, selectedMemberId, setSelectedMemberId, setMemberEvents, snapshot, tab]);

  useEffect(() => {
    const actorId = chatInboxActorId.trim();
    if (!activeRunIdForSelectedTeam || !actorId) {
      setInbox([]);
      return;
    }
    setInboxActorId(actorId);
    void loadInbox(actorId).catch((err) => {
      setError(parseError(err));
    });
  }, [
    activeRunIdForSelectedTeam,
    chatInboxActorId,
    loadInbox,
    parseError,
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
    conversationMessagesLength,
    markConversationSeen,
    scrollConversationToBottom,
    tab,
  ]);

  useEffect(() => {
    if (tab !== "mailbox") {
      return;
    }
    void loadMemberEvents("replace").catch((err) => {
      setError(parseError(err));
    });
  }, [loadMemberEvents, parseError, setError, tab]);
}
