import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import type { TeamActorMessageRecord, TeamConversationMessageRecord } from "../../api";
import type { TeamTab } from "./state";

type UseTeamConversationEffectsOptions = {
  token: string;
  selectedTeamId: string | null;
  selectedConversationId: string | null;
  tab: TeamTab;
  eventsAutoRefresh: boolean;
  refreshTaskMessages: (taskIdOverride?: string) => Promise<void>;
  setTaskMessages: Dispatch<SetStateAction<TeamConversationMessageRecord[]>>;
  setConversationMailboxMessages: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
};

export function useTeamConversationEffects({
  token,
  selectedTeamId,
  selectedConversationId,
  tab,
  eventsAutoRefresh,
  refreshTaskMessages,
  setTaskMessages,
  setConversationMailboxMessages,
}: UseTeamConversationEffectsOptions) {
  const refreshInFlightRef = useRef(false);
  const sseConnectedRef = useRef(false);

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
      !token.trim() ||
      !selectedTeamId ||
      !conversationId ||
      typeof EventSource === "undefined"
    ) {
      sseConnectedRef.current = false;
      return;
    }
    let cancelled = false;
    let source: EventSource | null = null;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;

    const clearReconnectTimer = () => {
      if (reconnectTimer != null) {
        window.clearTimeout(reconnectTimer);
        reconnectTimer = null;
      }
    };

    const closeSource = () => {
      sseConnectedRef.current = false;
      source?.close();
      source = null;
    };

    const scheduleReconnect = () => {
      if (cancelled) return;
      clearReconnectTimer();
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (cancelled) return;
        openSource();
      }, delay);
    };

    const openSource = () => {
      closeSource();
      const nextSource = new EventSource(
        `${location.origin}/sse/teams/${encodeURIComponent(
          selectedTeamId
        )}/tasks/${encodeURIComponent(conversationId)}/messages?token=${encodeURIComponent(
          token
        )}`
      );
      source = nextSource;
      nextSource.onopen = () => {
        reconnectAttempt = 0;
        sseConnectedRef.current = true;
      };
      nextSource.onmessage = (event) => {
        if (cancelled) return;
        if (event.data === "heartbeat") {
          return;
        }
        void refreshSelectedConversation().catch(() => undefined);
      };
      nextSource.onerror = () => {
        if (source !== nextSource) {
          nextSource.close();
          return;
        }
        closeSource();
        scheduleReconnect();
      };
    };

    openSource();
    return () => {
      cancelled = true;
      clearReconnectTimer();
      closeSource();
    };
  }, [
    eventsAutoRefresh,
    refreshSelectedConversation,
    selectedConversationId,
    selectedTeamId,
    tab,
    token,
  ]);

  useEffect(() => {
    const conversationId = selectedConversationId?.trim() ?? "";
    if (
      !eventsAutoRefresh ||
      tab !== "conversation" ||
      !token.trim() ||
      !selectedTeamId ||
      !conversationId
    ) {
      return;
    }
    const timer = window.setInterval(() => {
      if (sseConnectedRef.current) {
        return;
      }
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
    token,
  ]);
}
