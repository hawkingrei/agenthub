import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import type { TeamRunStatusFilter } from "./run_helpers";
import type { TeamRunBrowserState } from "./state";

type RefreshTeamRuns = (
  teamId: string,
  mode?: "replace" | "append",
  options?: {
    statusFilter?: TeamRunStatusFilter;
    beforeCreatedAt?: number;
  }
) => Promise<unknown>;

type UseTeamRunListActionsParams = {
  selectedTeamId: string | null;
  runStatusFilter: TeamRunStatusFilter;
  runsLoading: boolean;
  runsHasMore: boolean;
  runsBeforeCreatedAt?: number;
  setError: (value: string | null) => void;
  parseErrorMessage: (error: unknown) => string;
  setTeamRunBrowserByTeam: Dispatch<
    SetStateAction<Record<string, TeamRunBrowserState>>
  >;
  refreshTeamRuns: RefreshTeamRuns;
};

export function useTeamRunListActions({
  selectedTeamId,
  runStatusFilter,
  runsLoading,
  runsHasMore,
  runsBeforeCreatedAt,
  setError,
  parseErrorMessage,
  setTeamRunBrowserByTeam,
  refreshTeamRuns,
}: UseTeamRunListActionsParams) {
  const onRunStatusFilterChange = useCallback(
    (nextFilter: TeamRunStatusFilter) => {
      if (!selectedTeamId) return;
      setTeamRunBrowserByTeam((prev) => ({
        ...prev,
        [selectedTeamId]: {
          statusFilter: nextFilter,
          beforeCreatedAt: undefined,
          hasMore: false,
        },
      }));
    },
    [selectedTeamId, setTeamRunBrowserByTeam]
  );

  const onRefreshRuns = useCallback(async () => {
    if (!selectedTeamId) return;
    setError(null);
    try {
      await refreshTeamRuns(selectedTeamId, "replace", {
        statusFilter: runStatusFilter,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [
    parseErrorMessage,
    refreshTeamRuns,
    runStatusFilter,
    selectedTeamId,
    setError,
  ]);

  const onLoadMoreRuns = useCallback(async () => {
    if (!selectedTeamId || runsLoading || !runsHasMore) {
      return;
    }
    setError(null);
    try {
      await refreshTeamRuns(selectedTeamId, "append", {
        statusFilter: runStatusFilter,
        beforeCreatedAt: runsBeforeCreatedAt,
      });
    } catch (err) {
      setError(parseErrorMessage(err));
    }
  }, [
    parseErrorMessage,
    refreshTeamRuns,
    runStatusFilter,
    runsBeforeCreatedAt,
    runsHasMore,
    runsLoading,
    selectedTeamId,
    setError,
  ]);

  return {
    onRunStatusFilterChange,
    onRefreshRuns,
    onLoadMoreRuns,
  };
}
