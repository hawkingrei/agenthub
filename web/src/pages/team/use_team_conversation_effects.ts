import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import type { TeamActorMessageRecord, TeamConversationMessageRecord } from "../../api";
import type { SseConnectionState } from "../../connection_status";
import type { TeamTab } from "./state";
import { useResumeRefresh } from "./use_resume_refresh";

type UseTeamConversationEffectsOptions = {
  token: string;
  selectedTeamId: string | null;
  selectedConversationId: string | null;
  tab: TeamTab;
  eventsAutoRefresh: boolean;
  refreshTaskMessages: (taskIdOverride?: string) => Promise<void>;
  setTaskMessages: Dispatch<SetStateAction<TeamConversationMessageRecord[]>>;
  setConversationMailboxMessages: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
  onSseStateChange?: (nextState: SseConnectionState) => void;
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
  onSseStateChange,
}: UseTeamConversationEffectsOptions) {
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);
  const latestSelectionRef = useRef({ teamId: "", conversationId: "" });
  const refreshTaskMessagesRef = useRef(refreshTaskMessages);
  const sseConnectedRef = useRef(false);
  const updateSseState = useCallback(
    (nextState: SseConnectionState) => {
      onSseStateChange?.(nextState);
    },
    [onSseStateChange]
  );

  useEffect(() => {
    latestSelectionRef.current = {
      teamId: selectedTeamId?.trim() ?? "",
      conversationId: selectedConversationId?.trim() ?? "",
    };
  }, [selectedConversationId, selectedTeamId]);

  useEffect(() => {
    refreshTaskMessagesRef.current = refreshTaskMessages;
  }, [refreshTaskMessages]);

  const refreshSelectedConversation = useCallback(async () => {
    const { teamId, conversationId } = latestSelectionRef.current;
    if (!teamId || !conversationId) {
      return;
    }
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      return;
    }
    refreshInFlightRef.current = true;
    try {
      for (;;) {
        refreshQueuedRef.current = false;
        const current = latestSelectionRef.current;
        if (!current.teamId || !current.conversationId) {
          return;
        }
        await refreshTaskMessagesRef.current(current.conversationId);
        if (!refreshQueuedRef.current) {
          return;
        }
      }
    } finally {
      refreshInFlightRef.current = false;
      if (refreshQueuedRef.current) {
        refreshQueuedRef.current = false;
        void refreshSelectedConversation().catch(() => undefined);
      }
    }
  }, []);

  useEffect(() => {
    const conversationId = selectedConversationId?.trim() ?? "";
    if (!selectedTeamId || !conversationId) {
      setTaskMessages([]);
      setConversationMailboxMessages([]);
      updateSseState("idle");
      return;
    }
    void refreshSelectedConversation();
  }, [
    refreshSelectedConversation,
    selectedConversationId,
    selectedTeamId,
    setConversationMailboxMessages,
    setTaskMessages,
    updateSseState,
  ]);

  const conversationRefreshEnabled = Boolean(
    eventsAutoRefresh &&
      tab === "conversation" &&
      token.trim() &&
      (selectedTeamId?.trim() ?? "") &&
      (selectedConversationId?.trim() ?? "")
  );

  useResumeRefresh({
    enabled: conversationRefreshEnabled,
    intervalMs: 4000,
    refresh: refreshSelectedConversation,
  });

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
      updateSseState("idle");
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
      updateSseState("reconnecting");
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (cancelled) return;
        openSource();
      }, delay);
    };

    function openSource() {
      closeSource();
      updateSseState(reconnectAttempt > 0 ? "reconnecting" : "connecting");
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
        updateSseState("connected");
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
    }

    openSource();
    return () => {
      cancelled = true;
      clearReconnectTimer();
      closeSource();
      updateSseState("idle");
    };
  }, [
    eventsAutoRefresh,
    onSseStateChange,
    refreshSelectedConversation,
    selectedConversationId,
    selectedTeamId,
    tab,
    token,
    updateSseState,
  ]);

}
