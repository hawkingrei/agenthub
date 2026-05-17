import { useCallback, useReducer } from "react";
import {
  DEFAULT_TEAM_UI_STATE,
  reduceTeamUiState,
  type TeamTab,
} from "./state";

export function useTeamUiState() {
  const [state, dispatch] = useReducer(reduceTeamUiState, DEFAULT_TEAM_UI_STATE);

  const setTab = useCallback(
    (tab: TeamTab) => {
      dispatch({ type: "set_tab", tab });
    },
    [dispatch]
  );

  const setRunLookupId = useCallback(
    (runLookupId: string) => {
      dispatch({ type: "set_run_lookup_id", runLookupId });
    },
    [dispatch]
  );

  const setEventsAutoRefresh = useCallback(
    (eventsAutoRefresh: boolean) => {
      dispatch({ type: "set_events_auto_refresh", eventsAutoRefresh });
    },
    [dispatch]
  );

  return {
    ...state,
    setTab,
    setRunLookupId,
    setEventsAutoRefresh,
    dispatch,
  };
}
