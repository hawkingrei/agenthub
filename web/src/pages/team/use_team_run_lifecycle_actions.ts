import { useCallback } from "react";
import type { Dispatch, SetStateAction } from "react";
import { api, type TeamRunRecord } from "../../api";
import { upsertRun } from "./page_helpers";

type UseTeamRunLifecycleActionsParams = {
  token: string;
  activeRunId: string | null;
  runLookupId: string;
  setBusy: (value: string | null) => void;
  setError: (value: string | null) => void;
  parseErrorMessage: (error: unknown) => string;
  refreshRun: (runId: string) => Promise<TeamRunRecord>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<void>;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  setRuns: Dispatch<SetStateAction<TeamRunRecord[]>>;
  setSelectedTeamId: (teamId: string | null) => void;
  setActiveRunId: (runId: string | null) => void;
  setRunLookupId: (runId: string) => void;
};

export function useTeamRunLifecycleActions({
  token,
  activeRunId,
  runLookupId,
  setBusy,
  setError,
  parseErrorMessage,
  refreshRun,
  refreshEvents,
  refreshSnapshot,
  setRuns,
  setSelectedTeamId,
  setActiveRunId,
  setRunLookupId,
}: UseTeamRunLifecycleActionsParams) {
  const onLoadRunById = useCallback(async () => {
    const runId = runLookupId.trim();
    if (!runId) {
      setError("Run ID is required");
      return;
    }
    setBusy("load-run");
    setError(null);
    try {
      const run = await refreshRun(runId);
      setSelectedTeamId(run.team_id);
      setActiveRunId(run.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    parseErrorMessage,
    refreshRun,
    runLookupId,
    setActiveRunId,
    setBusy,
    setError,
    setSelectedTeamId,
  ]);

  const onCancelRun = useCallback(async () => {
    if (!activeRunId) return;
    setBusy("cancel-run");
    setError(null);
    try {
      const canceled = await api.cancelTeamRun(token, activeRunId);
      setRuns((prev) => upsertRun(prev, canceled));
      await Promise.all([refreshEvents(activeRunId), refreshSnapshot(activeRunId)]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    parseErrorMessage,
    refreshEvents,
    refreshSnapshot,
    setBusy,
    setError,
    setRuns,
    token,
  ]);

  const onResumeRun = useCallback(async () => {
    if (!activeRunId) return;
    setBusy("resume-run");
    setError(null);
    try {
      const resumed = await api.resumeTeamRun(token, activeRunId);
      setRuns((prev) => upsertRun(prev, resumed));
      setActiveRunId(resumed.id);
      setRunLookupId(resumed.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    parseErrorMessage,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    token,
  ]);

  const onRestartRun = useCallback(async () => {
    if (!activeRunId) return;
    setBusy("restart-run");
    setError(null);
    try {
      const restarted = await api.restartTeamRun(token, activeRunId);
      setRuns((prev) => upsertRun(prev, restarted));
      setActiveRunId(restarted.id);
      setRunLookupId(restarted.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    parseErrorMessage,
    setActiveRunId,
    setBusy,
    setError,
    setRunLookupId,
    setRuns,
    token,
  ]);

  return {
    onLoadRunById,
    onCancelRun,
    onResumeRun,
    onRestartRun,
  };
}
