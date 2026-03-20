import { useEffect } from "react";

type UseTeamTaskEffectsOptions = {
  selectedTeamId: string | null;
  enabled: boolean;
  refreshTasks: (teamId: string) => Promise<unknown>;
  onRefreshError?: (error: unknown) => void;
};

const TEAM_TASK_REFRESH_INTERVAL_MS = 10_000;

export function useTeamTaskEffects({
  selectedTeamId,
  enabled,
  refreshTasks,
  onRefreshError,
}: UseTeamTaskEffectsOptions) {
  useEffect(() => {
    const teamId = selectedTeamId?.trim() ?? "";
    if (!enabled || !teamId) {
      return;
    }

    const timer = window.setInterval(() => {
      void refreshTasks(teamId).catch((error) => {
        onRefreshError?.(error);
      });
    }, TEAM_TASK_REFRESH_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, onRefreshError, refreshTasks, selectedTeamId]);
}

export { TEAM_TASK_REFRESH_INTERVAL_MS };
