import { useCallback } from "react";
import { api } from "../../api";
import { parseOptionalJson } from "./create_helpers";
import type { StepAction } from "./state";

function parseCsvList(raw: string): string[] {
  return raw
    .split(",")
    .map((entry) => entry.trim())
    .filter((entry) => entry.length > 0);
}

type UseTeamStepActionsParams = {
  token: string;
  activeRunId: string | null;
  stepKey: string;
  stepMemberId: string;
  stepDependsOn: string;
  stepInput: string;
  selectedStepId: string;
  stepAction: StepAction;
  stepRemoteTaskId: string;
  stepOutput: string;
  stepFailText: string;
  stepInputReason: string;
  stepInputRequiredPayload: string;
  stepResumePayload: string;
  setBusy: (value: string | null) => void;
  setError: (value: string | null) => void;
  parseErrorMessage: (error: unknown) => string;
  refreshRun: (runId: string) => Promise<unknown>;
  refreshSteps: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<unknown>;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  setSelectedStepId: (value: string) => void;
};

export function useTeamStepActions({
  token,
  activeRunId,
  stepKey,
  stepMemberId,
  stepDependsOn,
  stepInput,
  selectedStepId,
  stepAction,
  stepRemoteTaskId,
  stepOutput,
  stepFailText,
  stepInputReason,
  stepInputRequiredPayload,
  stepResumePayload,
  setBusy,
  setError,
  parseErrorMessage,
  refreshRun,
  refreshSteps,
  refreshEvents,
  refreshSnapshot,
  setSelectedStepId,
}: UseTeamStepActionsParams) {
  const onSubmitStep = useCallback(async () => {
    if (!activeRunId) {
      setError("Select a run first");
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
      const created = await api.submitTeamRunStep(token, activeRunId, {
        step_key: stepKey.trim(),
        member_id: stepMemberId.trim(),
        depends_on: parseCsvList(stepDependsOn),
        input: parseOptionalJson(stepInput, "Step input"),
      });
      await Promise.all([
        refreshRun(activeRunId),
        refreshSteps(activeRunId),
        refreshEvents(activeRunId),
        refreshSnapshot(activeRunId),
      ]);
      setSelectedStepId(created.id);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    parseErrorMessage,
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
    token,
  ]);

  const onApplyStepAction = useCallback(async () => {
    if (!activeRunId) {
      setError("Select a run first");
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
        await api.startTeamRunStep(token, activeRunId, selectedStepId, {
          remote_task_id: stepRemoteTaskId.trim() || undefined,
        });
      } else if (stepAction === "complete") {
        await api.completeTeamRunStep(token, activeRunId, selectedStepId, {
          output: parseOptionalJson(stepOutput, "Step output"),
        });
      } else if (stepAction === "fail") {
        const errorText = stepFailText.trim();
        if (!errorText) {
          throw new Error("Fail reason is required");
        }
        await api.failTeamRunStep(token, activeRunId, selectedStepId, {
          error_text: errorText,
        });
      } else if (stepAction === "input_required") {
        await api.setTeamRunStepInputRequired(token, activeRunId, selectedStepId, {
          reason: stepInputReason.trim() || undefined,
          input: parseOptionalJson(stepInputRequiredPayload, "Input required payload"),
        });
      } else {
        await api.resumeTeamRunStep(token, activeRunId, selectedStepId, {
          input: parseOptionalJson(stepResumePayload, "Resume payload"),
        });
      }
      await Promise.all([
        refreshRun(activeRunId),
        refreshSteps(activeRunId),
        refreshEvents(activeRunId),
        refreshSnapshot(activeRunId),
      ]);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    parseErrorMessage,
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
    token,
  ]);

  return {
    onSubmitStep,
    onApplyStepAction,
  };
}
