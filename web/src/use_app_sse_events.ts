import { useState, useRef, useEffect } from "react";
import { AuthState } from "./types";
import { AgentRecord, OutputLine } from "./api";
import { 
  deriveConnectionBadge, 
  getNavigatorOnline, 
  OFFLINE_MESSAGE, 
  sanitizeErrorBannerMessage, 
  UPSTREAM_HTML_MESSAGE 
} from "./connection_status";
import { 
  getAdaptivePollInterval, 
  isSseConnectionStale, 
  shouldPollAgentEvents 
} from "./event_polling";
import { normalizeSseOutputLines } from "./app_live_output";
import { encodeSseTargetAgentIds, buildSseTargetAgentIds } from "./sse_targets";
import { shouldIgnoreAgentWsError } from "./agent_ws";
import { 
  clearAuthAndRedirect,
  isInvalidTokenMessage 
} from "./auth_redirect";

const SSE_STALE_RECONNECT_THRESHOLD_MS = 45_000;

export function useAppSseEvents(
  auth: AuthState | null,
  isAgentsRoute: boolean,
  agents: AgentRecord[],
  activeAgent: string | null,
  activeAgentStatus: string | null,
  consumeLiveOutputBatch: (lines: OutputLine[]) => void,
  loadAgentEvents: (id: string, sessionId?: string | null) => Promise<boolean>,

  refreshAgents: (opts?: { silent?: boolean }) => Promise<AgentRecord[] | null>
) {
  const token = auth?.token ?? null;
  const [networkOnline, setNetworkOnline] = useState<boolean>(getNavigatorOnline);
  const [sseState, setSseState] = useState<"idle" | "connecting" | "connected" | "reconnecting">("idle");
  const [error, setError] = useState<string | null>(null);

  const sseRef = useRef<EventSource | null>(null);
  const lastSseActivityAtRef = useRef<number>(Date.now());
  const activeAgentRef = useRef(activeAgent);
  const activeAgentStatusRef = useRef(activeAgentStatus);
  const requestSseReconnectRef = useRef<(() => void) | null>(null);

  const eventPollRef = useRef<{
    timer: number | null;
    idleCount: number;
    boostUntil: number | null;
  }>({
    timer: null,
    idleCount: 0,
    boostUntil: null,
  });

  useEffect(() => {
    activeAgentRef.current = activeAgent;
  }, [activeAgent]);

  useEffect(() => {
    activeAgentStatusRef.current = activeAgentStatus;
  }, [activeAgentStatus]);

  const streamAgentIds = buildSseTargetAgentIds(agents);
  const streamAgentIdsQuery = encodeSseTargetAgentIds(streamAgentIds);
  const hasSseTarget = streamAgentIds.length > 0;
  const connectionBadge = deriveConnectionBadge(networkOnline, hasSseTarget, sseState);

  useEffect(() => {
    if (!isAgentsRoute || !token) {
      setSseState("idle");
      requestSseReconnectRef.current = null;
      return;
    }
    const streamTarget = streamAgentIdsQuery;
    if (!hasSseTarget || streamTarget.length === 0) {
      setSseState("idle");
      requestSseReconnectRef.current = null;
      return;
    }

    let cancelled = false;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;

    const scheduleReconnect = () => {
      if (cancelled) return;
      if (reconnectTimer) window.clearTimeout(reconnectTimer);
      setSseState("reconnecting");
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (!cancelled) openSource();
      }, delay);
    };

    requestSseReconnectRef.current = scheduleReconnect;

    function openSource() {
      if (cancelled) return;
      setSseState("connecting");
      const source = new EventSource(
        `${location.origin}/sse/agents?ids=${encodeURIComponent(
          streamTarget
        )}&token=${encodeURIComponent(token!)}`
      );
      sseRef.current = source;

      source.onopen = () => {
        lastSseActivityAtRef.current = Date.now();
        reconnectAttempt = 0;
        setNetworkOnline(true);
        setSseState("connected");
      };

      source.onmessage = (event) => {
        lastSseActivityAtRef.current = Date.now();
        if (event.data === "heartbeat") return;
        try {
          const parsed = JSON.parse(event.data);
          const liveLines = normalizeSseOutputLines(parsed);
          if (liveLines.length > 0) {
            consumeLiveOutputBatch(liveLines);
          }
        } catch {
          if (typeof event.data === "string") {
            if (isInvalidTokenMessage(event.data)) {
              clearAuthAndRedirect();
              return;
            }
            const normalizedStreamError = sanitizeErrorBannerMessage(
              event.data,
              getNavigatorOnline()
            );
            if (!shouldIgnoreAgentWsError(normalizedStreamError, activeAgentStatusRef.current)) {
              setError(normalizedStreamError);
            }
          }
        }
      };

      source.onerror = () => {
        if (sseRef.current !== source) {
          source.close();
          return;
        }
        const online = getNavigatorOnline();
        setNetworkOnline(online);
        setSseState("reconnecting");
        source.close();
        sseRef.current = null;
        setError(online ? UPSTREAM_HTML_MESSAGE : OFFLINE_MESSAGE);
        
        void refreshAgents({ silent: true }).then((latest) => {
          if (cancelled) return;
          if (latest && buildSseTargetAgentIds(latest).length === 0) {
            setSseState("idle");
            return;
          }
          scheduleReconnect();
        });
      };
    }

    openSource();

    return () => {
      cancelled = true;
      if (reconnectTimer) window.clearTimeout(reconnectTimer);
      requestSseReconnectRef.current = null;
      if (sseRef.current) {
        sseRef.current.close();
        sseRef.current = null;
      }
      setSseState("idle");
    };
  }, [isAgentsRoute, token, streamAgentIdsQuery, hasSseTarget, consumeLiveOutputBatch, refreshAgents]);

  useEffect(() => {
    if (!isAgentsRoute || !token || !activeAgent) {
      if (eventPollRef.current.timer) window.clearTimeout(eventPollRef.current.timer);
      eventPollRef.current.idleCount = 0;
      return;
    }

    let cancelled = false;
    const schedulePoll = (delay: number) => {
      if (cancelled) return;
      if (eventPollRef.current.timer) window.clearTimeout(eventPollRef.current.timer);

      const now = Date.now();
      const isSseOpen = sseRef.current?.readyState === EventSource.OPEN;
      const sseStale = isSseConnectionStale(Boolean(isSseOpen), lastSseActivityAtRef.current, now, SSE_STALE_RECONNECT_THRESHOLD_MS);

      if (isSseOpen && sseStale) {
        sseRef.current?.close();
        sseRef.current = null;
        setSseState("reconnecting");
        requestSseReconnectRef.current?.();
      }

      if (isSseOpen && !eventPollRef.current.boostUntil && !sseStale) {
        eventPollRef.current.timer = null;
        return;
      }

      const nextDelay = eventPollRef.current.boostUntil && eventPollRef.current.boostUntil > now ? 1000 : delay;
      eventPollRef.current.timer = window.setTimeout(async () => {
        if (cancelled) return;
        const shouldPoll = shouldPollAgentEvents(Boolean(sseRef.current?.readyState === EventSource.OPEN), eventPollRef.current.boostUntil, Date.now(), sseStale);
        
        let hasNew = false;
        if (shouldPoll) {
          hasNew = await loadAgentEvents(activeAgentRef.current!, null);
        }

        if (hasNew) {
          eventPollRef.current.idleCount = 0;
        } else if (shouldPoll) {
          eventPollRef.current.idleCount += 1;
        }

        schedulePoll(getAdaptivePollInterval(eventPollRef.current.idleCount));
      }, nextDelay);
    };

    const pollState = eventPollRef.current;
    schedulePoll(getAdaptivePollInterval(pollState.idleCount));

    return () => {
      cancelled = true;
      if (pollState.timer) window.clearTimeout(pollState.timer);
    };
  }, [isAgentsRoute, token, activeAgent, loadAgentEvents]);

  return {
    networkOnline,
    sseState,
    connectionBadge,
    error,
    setError,
    eventPollRef,
  };
}
