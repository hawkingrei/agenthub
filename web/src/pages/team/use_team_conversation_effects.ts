import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import type { TeamActorMessageRecord, TeamConversationMessageRecord } from "../../api";
import type { TeamTab } from "./state";

type UseTeamConversationEffectsOptions = {
  selectedTeamId: string | null;
  selectedConversationId: string | null;
  tab: TeamTab;
  eventsAutoRefresh: boolean;
  refreshTaskMessages: (taskIdOverride?: string) => Promise<void>;
  setTaskMessages: Dispatch<SetStateAction<TeamConversationMessageRecord[]>>;
  setConversationMailboxMessages: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
};

export function useTeamConversationEffects({
  selectedTeamId,
  selectedConversationId,
  tab,
  eventsAutoRefresh,
  refreshTaskMessages,
  setTaskMessages,
  setConversationMailboxMessages,
}: UseTeamConversationEffectsOptions) {
  const refreshInFlightRef = useRef(false);

  const refreshSelectedConversation = useCallback(async () => {
    const conversationId = selectedConversationId?.trim() ?? "";
    if (!selectedTeamId || !conversationId || refreshInFlightRef.current) {
      return;
    }
    refreshInFlightRef.current = true;
    try {
      await refreshTaskMessages(conversationId);
    } finally {
      refreshInFlightRef.current = false;
    }
  }, [refreshTaskMessages, selectedConversationId, selectedTeamId]);

  useEffect(() => {
    const conversationId = selectedConversationId?.trim() ?? "";
    if (!selectedTeamId || !conversationId) {
      setTaskMessages([]);
      setConversationMailboxMessages([]);
      return;
    }
    void refreshSelectedConversation();
  }, [
    refreshSelectedConversation,
    selectedConversationId,
    selectedTeamId,
    setConversationMailboxMessages,
    setTaskMessages,
  ]);

  useEffect(() => {
    const conversationId = selectedConversationId?.trim() ?? "";
    if (
      !eventsAutoRefresh ||
      tab !== "conversation" ||
      !selectedTeamId ||
      !conversationId
    ) {
      return;
    }
    const timer = window.setInterval(() => {
      void refreshSelectedConversation().catch(() => undefined);
    }, 4000);
    return () => {
      window.clearInterval(timer);
    };
  }, [
    eventsAutoRefresh,
    refreshSelectedConversation,
    selectedConversationId,
    selectedTeamId,
    tab,
  ]);
}
