import { useCallback } from "react";
import { useResumeRefresh } from "./use_resume_refresh";

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
  const teamId = selectedTeamId?.trim() ?? "";
  const taskRefreshEnabled = enabled && teamId.length > 0;
  const refresh = useCallback(async () => {
    await refreshTasks(teamId);
  }, [refreshTasks, teamId]);

  useResumeRefresh({
    enabled: taskRefreshEnabled,
    intervalMs: TEAM_TASK_REFRESH_INTERVAL_MS,
    pauseWhenHidden: true,
    pauseWhenHiddenAfterInitialRefresh: true,
    initialRefreshKey: teamId,
    refresh,
    onRefreshError,
  });
}

export { TEAM_TASK_REFRESH_INTERVAL_MS };
