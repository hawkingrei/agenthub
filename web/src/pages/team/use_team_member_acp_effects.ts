import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import type { AgentEvent } from "../../api";
import { normalizeSseOutputLines } from "../../app_live_output";
import { upsertAgentEventList } from "./page_helpers";
import type { TeamTab } from "./state";
import { useResumeRefresh } from "./use_resume_refresh";

type UseTeamMemberAcpEffectsOptions = {
  token: string;
  selectedAgentId: string;
  selectedSessionId: string | null;
  tab: TeamTab;
  eventsAutoRefresh: boolean;
  loadMemberEvents: (
    mode?: "replace" | "prepend",
    sessionIdOverride?: string | null
  ) => Promise<void>;
  setMemberEvents: Dispatch<SetStateAction<AgentEvent[]>>;
  setMemberEventsHasMore: Dispatch<SetStateAction<boolean>>;
  onLiveActivity?: () => Promise<void> | void;
};

const TEAM_MEMBER_ACP_POLL_INTERVAL_MS = 4000;
const TEAM_MEMBER_ACP_ACTIVITY_SYNC_DEBOUNCE_MS = 500;
const TEAM_MEMBER_ACP_SELECTION_REFRESH_DEDUPE_MS = 1500;

function isMemberAcpTab(tab: TeamTab): boolean {
  return tab === "agent_acp" || tab === "member_console";
}

export function useTeamMemberAcpEffects({
  token,
  selectedAgentId,
  selectedSessionId,
  tab,
  eventsAutoRefresh,
  loadMemberEvents,
  setMemberEvents,
  setMemberEventsHasMore,
  onLiveActivity,
}: UseTeamMemberAcpEffectsOptions) {
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);
  const refreshQueuedReasonRef = useRef<"selection" | "poll">("selection");
  const latestSelectionRef = useRef({ agentId: "", sessionId: "" });
  const loadMemberEventsRef = useRef(loadMemberEvents);
  const sseConnectedRef = useRef(false);
  const sseConnectingRef = useRef(false);
  const activitySyncInFlightRef = useRef(false);
  const activitySyncQueuedRef = useRef(false);
  const activitySyncTimerRef = useRef<number | null>(null);
  const pollFallbackAllowedRef = useRef(false);
  const [pollFallbackEnabled, setPollFallbackEnabled] = useState(false);
  const onLiveActivityRef = useRef(onLiveActivity);
  const lastSelectionRefreshKeyRef = useRef<string | null>(null);
  const lastSelectionRefreshAtRef = useRef<number>(0);

  useEffect(() => {
    latestSelectionRef.current = {
      agentId: selectedAgentId.trim(),
      sessionId: selectedSessionId?.trim() ?? "",
    };
  }, [selectedAgentId, selectedSessionId]);

  useEffect(() => {
    pollFallbackAllowedRef.current = Boolean(
      eventsAutoRefresh &&
        isMemberAcpTab(tab) &&
        selectedAgentId.trim() &&
        (selectedSessionId?.trim() ?? "")
    );
  }, [eventsAutoRefresh, selectedAgentId, selectedSessionId, tab]);

  useEffect(() => {
    loadMemberEventsRef.current = loadMemberEvents;
  }, [loadMemberEvents]);

  useEffect(() => {
    onLiveActivityRef.current = onLiveActivity;
  }, [onLiveActivity]);

  const syncPollFallbackEnabled = useCallback(() => {
    const nextEnabled =
      pollFallbackAllowedRef.current &&
      (typeof EventSource === "undefined" ||
        (!sseConnectedRef.current && !sseConnectingRef.current));
    setPollFallbackEnabled((previous) =>
      previous === nextEnabled ? previous : nextEnabled
    );
  }, []);

  const refreshSelectedMemberEvents = useCallback(async (reason: "selection" | "poll" = "selection") => {
    const { agentId, sessionId } = latestSelectionRef.current;
    if (!agentId || !sessionId) {
      return;
    }
    if (reason === "selection") {
      const selectionKey = `${agentId}:${sessionId}`;
      const now =
        typeof performance !== "undefined" && typeof performance.now === "function"
          ? performance.now()
          : Date.now();
      if (
        lastSelectionRefreshKeyRef.current === selectionKey &&
        now - lastSelectionRefreshAtRef.current < TEAM_MEMBER_ACP_SELECTION_REFRESH_DEDUPE_MS
      ) {
        return;
      }
      lastSelectionRefreshKeyRef.current = selectionKey;
      lastSelectionRefreshAtRef.current = now;
    }
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      refreshQueuedReasonRef.current = reason;
      return;
    }
    refreshInFlightRef.current = true;
    try {
      for (;;) {
        refreshQueuedRef.current = false;
        const current = latestSelectionRef.current;
        if (!current.agentId || !current.sessionId) {
          return;
        }
        await loadMemberEventsRef.current("replace");
        if (!refreshQueuedRef.current) {
          return;
        }
      }
    } finally {
      refreshInFlightRef.current = false;
      if (refreshQueuedRef.current) {
        refreshQueuedRef.current = false;
        const queuedReason = refreshQueuedReasonRef.current;
        void refreshSelectedMemberEvents(queuedReason).catch(() => undefined);
      }
    }
  }, []);

  const scheduleLiveActivitySync = useCallback(() => {
    if (!onLiveActivityRef.current) {
      return;
    }
    if (activitySyncTimerRef.current != null) {
      window.clearTimeout(activitySyncTimerRef.current);
    }
    activitySyncTimerRef.current = window.setTimeout(() => {
      activitySyncTimerRef.current = null;
      const runSync = async () => {
        if (activitySyncInFlightRef.current) {
          activitySyncQueuedRef.current = true;
          return;
        }
        const callback = onLiveActivityRef.current;
        if (!callback) {
          return;
        }
        activitySyncInFlightRef.current = true;
        try {
          for (;;) {
            activitySyncQueuedRef.current = false;
            await callback();
            if (!activitySyncQueuedRef.current) {
              return;
            }
          }
        } finally {
          activitySyncInFlightRef.current = false;
        }
      };
      void runSync().catch(() => undefined);
    }, TEAM_MEMBER_ACP_ACTIVITY_SYNC_DEBOUNCE_MS);
  }, []);

  useEffect(() => {
    if (!isMemberAcpTab(tab) || !selectedAgentId.trim()) {
      return;
    }
    if (!(selectedSessionId?.trim() ?? "")) {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      setMemberEvents([]);
      setMemberEventsHasMore(false);
      setPollFallbackEnabled(false);
      return;
    }
    sseConnectingRef.current = Boolean(
      eventsAutoRefresh &&
        token.trim() &&
        typeof EventSource !== "undefined" &&
        selectedAgentId.trim() &&
        (selectedSessionId?.trim() ?? "") &&
        isMemberAcpTab(tab)
    );
    syncPollFallbackEnabled();
    void refreshSelectedMemberEvents("selection");
  }, [
    eventsAutoRefresh,
    refreshSelectedMemberEvents,
    selectedAgentId,
    selectedSessionId,
    setMemberEvents,
    setMemberEventsHasMore,
    syncPollFallbackEnabled,
    tab,
    token,
  ]);

  const memberAcpSyncEnabled = Boolean(
    eventsAutoRefresh &&
      token.trim() &&
      selectedAgentId.trim() &&
      (selectedSessionId?.trim() ?? "") &&
      isMemberAcpTab(tab)
  );
  const memberAcpRefreshEnabled = memberAcpSyncEnabled && pollFallbackEnabled;

  useResumeRefresh({
    enabled: memberAcpRefreshEnabled,
    intervalMs: TEAM_MEMBER_ACP_POLL_INTERVAL_MS,
    refresh: () => refreshSelectedMemberEvents("poll"),
  });

  useEffect(() => {
    const agentId = selectedAgentId.trim();
    const sessionId = selectedSessionId?.trim() ?? "";
    if (!memberAcpSyncEnabled || typeof EventSource === "undefined") {
      sseConnectedRef.current = false;
      sseConnectingRef.current = false;
      syncPollFallbackEnabled();
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
      sseConnectingRef.current = false;
      source?.close();
      source = null;
    };

    const resetMemberAcpStream = ({
      clearTimer = true,
      disablePolling = false,
    }: {
      clearTimer?: boolean;
      disablePolling?: boolean;
    } = {}) => {
      if (clearTimer) {
        clearReconnectTimer();
      }
      closeSource();
      if (disablePolling) {
        setPollFallbackEnabled(false);
        return;
      }
      syncPollFallbackEnabled();
    };

    const scheduleReconnect = () => {
      if (cancelled) {
        return;
      }
      resetMemberAcpStream();
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (cancelled) {
          return;
        }
        openSource();
      }, delay);
    };

    function openSource() {
      resetMemberAcpStream({ clearTimer: false });
      sseConnectingRef.current = true;
      syncPollFallbackEnabled();
      const nextSource = new EventSource(
        `${location.origin}/sse/agents?ids=${encodeURIComponent(agentId)}&token=${encodeURIComponent(
          token
        )}`
      );
      source = nextSource;
      nextSource.onopen = () => {
        reconnectAttempt = 0;
        sseConnectingRef.current = false;
        sseConnectedRef.current = true;
        syncPollFallbackEnabled();
      };
      nextSource.onmessage = (event) => {
        if (cancelled || event.data === "heartbeat") {
          return;
        }
        let parsed: unknown;
        try {
          parsed = JSON.parse(event.data);
        } catch {
          return;
        }
        const liveLines = normalizeSseOutputLines(parsed).filter(
          (line) => line.agent_id === agentId && line.session_id === sessionId
        );
        if (liveLines.length === 0) {
          return;
        }
        setMemberEvents((prev) =>
          upsertAgentEventList(prev, liveLines, "replace", sessionId)
        );
        scheduleLiveActivitySync();
      };
      nextSource.onerror = () => {
        if (source !== nextSource) {
          nextSource.close();
          return;
        }
        resetMemberAcpStream({ clearTimer: false });
        scheduleReconnect();
      };
    }

    openSource();
    return () => {
      cancelled = true;
      if (activitySyncTimerRef.current != null) {
        window.clearTimeout(activitySyncTimerRef.current);
        activitySyncTimerRef.current = null;
      }
      resetMemberAcpStream({ disablePolling: true });
    };
  }, [
    scheduleLiveActivitySync,
    memberAcpSyncEnabled,
    selectedAgentId,
    selectedSessionId,
    setMemberEvents,
    syncPollFallbackEnabled,
    token,
  ]);
}

export { TEAM_MEMBER_ACP_POLL_INTERVAL_MS };
