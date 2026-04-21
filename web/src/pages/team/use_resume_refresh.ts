import { useCallback, useEffect, useRef } from "react";

type UseResumeRefreshOptions = {
  enabled: boolean;
  intervalMs?: number | null;
  pauseWhenHidden?: boolean;
  refresh: () => Promise<unknown>;
  onRefreshError?: (error: unknown) => void;
};

export function useResumeRefresh({
  enabled,
  intervalMs,
  pauseWhenHidden = false,
  refresh,
  onRefreshError,
}: UseResumeRefreshOptions) {
  const enabledRef = useRef(enabled);
  const pauseWhenHiddenRef = useRef(pauseWhenHidden);
  const refreshRef = useRef(refresh);
  const onRefreshErrorRef = useRef(onRefreshError);
  const refreshInFlightRef = useRef(false);
  const refreshQueuedRef = useRef(false);

  useEffect(() => {
    enabledRef.current = enabled;
  }, [enabled]);

  useEffect(() => {
    pauseWhenHiddenRef.current = pauseWhenHidden;
  }, [pauseWhenHidden]);

  useEffect(() => {
    refreshRef.current = refresh;
  }, [refresh]);

  useEffect(() => {
    onRefreshErrorRef.current = onRefreshError;
  }, [onRefreshError]);

  const requestRefresh = useCallback(() => {
    if (!enabledRef.current) {
      return;
    }
    if (
      pauseWhenHiddenRef.current &&
      typeof document !== "undefined" &&
      document.visibilityState !== "visible"
    ) {
      return;
    }
    if (refreshInFlightRef.current) {
      refreshQueuedRef.current = true;
      return;
    }
    refreshInFlightRef.current = true;
    void (async () => {
      let shouldRestart = false;
      try {
        do {
          refreshQueuedRef.current = false;
          await refreshRef.current();
        } while (enabledRef.current && refreshQueuedRef.current);
      } catch (error) {
        onRefreshErrorRef.current?.(error);
      } finally {
        refreshInFlightRef.current = false;
        if (!enabledRef.current) {
          refreshQueuedRef.current = false;
        } else if (refreshQueuedRef.current) {
          refreshQueuedRef.current = false;
          shouldRestart = true;
        }
      }
      if (shouldRestart) {
        requestRefresh();
      }
    })();
  }, []);

  useEffect(() => {
    if (!enabled) {
      return;
    }

    const handleFocus = () => {
      requestRefresh();
    };
    const handleOnline = () => {
      requestRefresh();
    };
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        requestRefresh();
      }
    };

    window.addEventListener("focus", handleFocus);
    window.addEventListener("online", handleOnline);
    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      window.removeEventListener("focus", handleFocus);
      window.removeEventListener("online", handleOnline);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [enabled, requestRefresh]);

  useEffect(() => {
    if (!enabled || intervalMs == null || intervalMs <= 0) {
      return;
    }
    const timer = window.setInterval(() => {
      requestRefresh();
    }, intervalMs);
    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, intervalMs, requestRefresh]);
}
