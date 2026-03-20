import { useEffect } from "react";

type UseTeamRuntimeEffectsOptions = {
  selectedTeamId: string | null;
  enabled: boolean;
  refreshTeamRuntime: (teamId: string) => Promise<unknown>;
  onRefreshError?: (error: unknown) => void;
};

const TEAM_RUNTIME_REFRESH_INTERVAL_MS = 60_000;

export function useTeamRuntimeEffects({
  selectedTeamId,
  enabled,
  refreshTeamRuntime,
  onRefreshError,
}: UseTeamRuntimeEffectsOptions) {
  useEffect(() => {
    const teamId = selectedTeamId?.trim() ?? "";
    if (!enabled || !teamId) {
      return;
    }

    const timer = window.setInterval(() => {
      void refreshTeamRuntime(teamId).catch((error) => {
        onRefreshError?.(error);
      });
    }, TEAM_RUNTIME_REFRESH_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, onRefreshError, refreshTeamRuntime, selectedTeamId]);
}

export { TEAM_RUNTIME_REFRESH_INTERVAL_MS };
