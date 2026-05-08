import { useCallback, useEffect, useRef, useState } from "react";
import { buildTeamRuntimeSseUrl } from "../../api";
import { useResumeRefresh } from "./use_resume_refresh";

type UseTeamRuntimeEffectsOptions = {
  token: string;
  selectedTeamId: string | null;
  enabled: boolean;
  refreshTeamRuntime: (teamId: string) => Promise<unknown>;
  onRefreshError?: (error: unknown) => void;
};

const TEAM_RUNTIME_REFRESH_INTERVAL_MS = 60_000;

export function useTeamRuntimeEffects({
  token,
  selectedTeamId,
  enabled,
  refreshTeamRuntime,
  onRefreshError,
}: UseTeamRuntimeEffectsOptions) {
  const teamId = selectedTeamId?.trim() ?? "";
  const runtimeRefreshEnabled = enabled && teamId.length > 0;
  const [pollFallbackEnabled, setPollFallbackEnabled] = useState(false);
  const onRefreshErrorRef = useRef(onRefreshError);

  useEffect(() => {
    onRefreshErrorRef.current = onRefreshError;
  }, [onRefreshError]);

  const refresh = useCallback(async () => {
    await refreshTeamRuntime(teamId);
  }, [refreshTeamRuntime, teamId]);

  useEffect(() => {
    if (!runtimeRefreshEnabled) {
      setPollFallbackEnabled(false);
      return;
    }
    if (typeof EventSource === "undefined" || !token.trim()) {
      setPollFallbackEnabled(true);
      return;
    }

    let cancelled = false;
    let source: EventSource | null = null;
    let reconnectTimer: number | null = null;
    let reconnectAttempt = 0;

    const closeSource = () => {
      source?.close();
      source = null;
    };

    const scheduleReconnect = () => {
      if (cancelled) return;
      closeSource();
      setPollFallbackEnabled(true);
      const delay = Math.min(30_000, 1000 * 2 ** reconnectAttempt);
      reconnectAttempt = Math.min(reconnectAttempt + 1, 6);
      reconnectTimer = window.setTimeout(() => {
        if (!cancelled) {
          openSource();
        }
      }, delay);
    };

    function openSource() {
      closeSource();
      const nextSource = new EventSource(
        buildTeamRuntimeSseUrl(location.origin, teamId, token)
      );
      source = nextSource;
      nextSource.onopen = () => {
        reconnectAttempt = 0;
        setPollFallbackEnabled(false);
      };
      nextSource.onmessage = (event) => {
        if (cancelled || event.data === "heartbeat") {
          return;
        }
        void refreshTeamRuntime(teamId).catch((error) => {
          onRefreshErrorRef.current?.(error);
        });
      };
      nextSource.onerror = () => {
        if (source !== nextSource) {
          nextSource.close();
          return;
        }
        scheduleReconnect();
      };
    }

    openSource();
    return () => {
      cancelled = true;
      if (reconnectTimer != null) {
        window.clearTimeout(reconnectTimer);
      }
      closeSource();
      setPollFallbackEnabled(false);
    };
  }, [refreshTeamRuntime, runtimeRefreshEnabled, teamId, token]);

  useResumeRefresh({
    enabled: runtimeRefreshEnabled,
    intervalMs: pollFallbackEnabled ? TEAM_RUNTIME_REFRESH_INTERVAL_MS : null,
    pauseWhenHidden: true,
    pauseWhenHiddenAfterInitialRefresh: true,
    initialRefreshKey: teamId,
    refresh,
    onRefreshError,
  });
}

export { TEAM_RUNTIME_REFRESH_INTERVAL_MS };
