import { useCallback } from "react";
import { useResumeRefresh } from "./use_resume_refresh";

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
  const teamId = selectedTeamId?.trim() ?? "";
  const runtimeRefreshEnabled = enabled && teamId.length > 0;
  const refresh = useCallback(async () => {
    await refreshTeamRuntime(teamId);
  }, [refreshTeamRuntime, teamId]);

  useResumeRefresh({
    enabled: runtimeRefreshEnabled,
    intervalMs: TEAM_RUNTIME_REFRESH_INTERVAL_MS,
    pauseWhenHidden: true,
    refresh,
    onRefreshError,
  });
}

export { TEAM_RUNTIME_REFRESH_INTERVAL_MS };
