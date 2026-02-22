import { useCallback, useMemo, type Dispatch, type SetStateAction } from "react";
import { api, type TeamRunSnapshotRecord, type TeamRunRecord, type TeamStepRecord } from "../../api";
import { parseErrorMessage, parseOptionalJson } from "./create_helpers";
import type { StepAction } from "./state";

type UseTeamStepActionsOptions = {
  token: string;
  activeRunIdForSelectedTeam: string | null;
  selectedStepId: string;
  stepAction: StepAction;
  stepKey: string;
  stepMemberId: string;
  stepDependsOn: string;
  stepInput: string;
  stepRemoteTaskId: string;
  stepOutput: string;
  stepFailText: string;
  stepInputReason: string;
  stepInputRequiredPayload: string;
  stepResumePayload: string;
  setBusy: Dispatch<SetStateAction<string | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setSelectedStepId: (next: string) => void;
  refreshRun: (runId: string) => Promise<TeamRunRecord>;
  refreshSteps: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<void>;
  refreshSnapshot: (runId: string) => Promise<TeamRunSnapshotRecord>;
};

type TeamStepApiClient = {
  submitTeamRunStep: (
    runId: string,
    payload: {
      step_key: string;
      member_id: string;
      depends_on?: string[];
      input?: unknown;
    }
  ) => Promise<TeamStepRecord>;
  startTeamRunStep: (
    runId: string,
    stepId: string,
    payload: { remote_task_id?: string }
  ) => Promise<TeamStepRecord>;
  completeTeamRunStep: (
    runId: string,
    stepId: string,
    payload: { output?: unknown }
  ) => Promise<TeamStepRecord>;
  failTeamRunStep: (
    runId: string,
    stepId: string,
    payload: { error_text: string }
  ) => Promise<TeamStepRecord>;
  setTeamRunStepInputRequired: (
    runId: string,
    stepId: string,
    payload: { reason?: string; input?: unknown }
  ) => Promise<TeamStepRecord>;
  resumeTeamRunStep: (
    runId: string,
    stepId: string,
    payload: { input?: unknown }
  ) => Promise<TeamStepRecord>;
};

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function buildTeamStepApiClient(token: string): TeamStepApiClient {
  return {
    submitTeamRunStep: (runId, payload) => api.submitTeamRunStep(token, runId, payload),
    startTeamRunStep: (runId, stepId, payload) =>
      api.startTeamRunStep(token, runId, stepId, payload),
    completeTeamRunStep: (runId, stepId, payload) =>
      api.completeTeamRunStep(token, runId, stepId, payload),
    failTeamRunStep: (runId, stepId, payload) =>
      api.failTeamRunStep(token, runId, stepId, payload),
    setTeamRunStepInputRequired: (runId, stepId, payload) =>
      api.setTeamRunStepInputRequired(token, runId, stepId, payload),
    resumeTeamRunStep: (runId, stepId, payload) =>
      api.resumeTeamRunStep(token, runId, stepId, payload),
  };
}

export function useTeamStepActions(options: UseTeamStepActionsOptions) {
  const {
    token,
    activeRunIdForSelectedTeam,
    selectedStepId,
    stepAction,
    stepKey,
    stepMemberId,
    stepDependsOn,
    stepInput,
    stepRemoteTaskId,
    stepOutput,
    stepFailText,
    stepInputReason,
    stepInputRequiredPayload,
    stepResumePayload,
    setBusy,
    setError,
    setSelectedStepId,
    refreshRun,
    refreshSteps,
    refreshEvents,
    refreshSnapshot,
  } = options;

  const teamStepApi = useMemo(() => buildTeamStepApiClient(token), [token]);

  const onSubmitStep = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    if (!stepKey.trim()) {
      setError("step_key is required");
      return;
    }
    if (!stepMemberId.trim()) {
      setError("member_id is required");
      return;
    }
    setBusy("submit-step");
    setError(null);
    try {
      const created = await teamStepApi.submitTeamRunStep(activeRunIdForSelectedTeam, {
        step_key: stepKey.trim(),
        member_id: stepMemberId.trim(),
        depends_on: parseCsvList(stepDependsOn),
        input: parseOptionalJson(stepInput, "Step input"),
      });
      await Promise.all([
        refreshRun(activeRunIdForSelectedTeam),
        refreshSteps(activeRunIdForSelectedTeam),
        refreshEvents(activeRunIdForSelectedTeam),
        refreshSnapshot(activeRunIdForSelectedTeam),
      ]);
      setSelectedStepId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunIdForSelectedTeam,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    refreshSteps,
    setBusy,
    setError,
    setSelectedStepId,
    stepDependsOn,
    stepInput,
    stepKey,
    stepMemberId,
    teamStepApi,
  ]);

  const onApplyStepAction = useCallback(async () => {
    if (!activeRunIdForSelectedTeam) {
      setError("Select a run in the current team first");
      return;
    }
    if (!selectedStepId) {
      setError("Select a step first");
      return;
    }
    setBusy(`step-${stepAction}`);
    setError(null);
    try {
      if (stepAction === "start") {
        await teamStepApi.startTeamRunStep(activeRunIdForSelectedTeam, selectedStepId, {
          remote_task_id: stepRemoteTaskId.trim() || undefined,
        });
      } else if (stepAction === "complete") {
        await teamStepApi.completeTeamRunStep(activeRunIdForSelectedTeam, selectedStepId, {
          output: parseOptionalJson(stepOutput, "Step output"),
        });
      } else if (stepAction === "fail") {
        const errorText = stepFailText.trim();
        if (!errorText) {
          throw new Error("Fail reason is required");
        }
        await teamStepApi.failTeamRunStep(activeRunIdForSelectedTeam, selectedStepId, {
          error_text: errorText,
        });
      } else if (stepAction === "input_required") {
        await teamStepApi.setTeamRunStepInputRequired(activeRunIdForSelectedTeam, selectedStepId, {
          reason: stepInputReason.trim() || undefined,
          input: parseOptionalJson(stepInputRequiredPayload, "Input required payload"),
        });
      } else {
        await teamStepApi.resumeTeamRunStep(activeRunIdForSelectedTeam, selectedStepId, {
          input: parseOptionalJson(stepResumePayload, "Resume payload"),
        });
      }

      await Promise.all([
        refreshRun(activeRunIdForSelectedTeam),
        refreshSteps(activeRunIdForSelectedTeam),
        refreshEvents(activeRunIdForSelectedTeam),
        refreshSnapshot(activeRunIdForSelectedTeam),
      ]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunIdForSelectedTeam,
    refreshEvents,
    refreshRun,
    refreshSnapshot,
    refreshSteps,
    selectedStepId,
    setBusy,
    setError,
    stepAction,
    stepFailText,
    stepInputReason,
    stepInputRequiredPayload,
    stepOutput,
    stepRemoteTaskId,
    stepResumePayload,
    teamStepApi,
  ]);

  return {
    onSubmitStep,
    onApplyStepAction,
  };
}
