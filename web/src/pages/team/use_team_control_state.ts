import { useCallback, useReducer } from "react";
import {
  DEFAULT_TEAM_CONTROL_STATE,
  reduceTeamControlState,
  type StepAction,
  type TeamControlState,
} from "./state";

export function useTeamControlState() {
  const [state, dispatch] = useReducer(reduceTeamControlState, DEFAULT_TEAM_CONTROL_STATE);

  const patchTeamControl = useCallback(
    (patch: Partial<TeamControlState>) => {
      dispatch({ type: "patch", patch });
    },
    [dispatch]
  );

  const setRunContextId = useCallback(
    (next: string) => patchTeamControl({ runContextId: next }),
    [patchTeamControl]
  );

  const setRunInput = useCallback(
    (next: string) => patchTeamControl({ runInput: next }),
    [patchTeamControl]
  );

  const setStepKey = useCallback(
    (next: string) => patchTeamControl({ stepKey: next }),
    [patchTeamControl]
  );

  const setStepMemberId = useCallback(
    (next: string) => patchTeamControl({ stepMemberId: next }),
    [patchTeamControl]
  );

  const setStepDependsOn = useCallback(
    (next: string) => patchTeamControl({ stepDependsOn: next }),
    [patchTeamControl]
  );

  const setStepInput = useCallback(
    (next: string) => patchTeamControl({ stepInput: next }),
    [patchTeamControl]
  );

  const setSelectedStepId = useCallback(
    (next: string) => patchTeamControl({ selectedStepId: next }),
    [patchTeamControl]
  );

  const setStepAction = useCallback(
    (next: StepAction) => patchTeamControl({ stepAction: next }),
    [patchTeamControl]
  );

  const setStepRemoteTaskId = useCallback(
    (next: string) => patchTeamControl({ stepRemoteTaskId: next }),
    [patchTeamControl]
  );

  const setStepOutput = useCallback(
    (next: string) => patchTeamControl({ stepOutput: next }),
    [patchTeamControl]
  );

  const setStepFailText = useCallback(
    (next: string) => patchTeamControl({ stepFailText: next }),
    [patchTeamControl]
  );

  const setStepInputReason = useCallback(
    (next: string) => patchTeamControl({ stepInputReason: next }),
    [patchTeamControl]
  );

  const setStepInputRequiredPayload = useCallback(
    (next: string) => patchTeamControl({ stepInputRequiredPayload: next }),
    [patchTeamControl]
  );

  const setStepResumePayload = useCallback(
    (next: string) => patchTeamControl({ stepResumePayload: next }),
    [patchTeamControl]
  );

  return {
    ...state,
    patchTeamControl,
    setRunContextId,
    setRunInput,
    setStepKey,
    setStepMemberId,
    setStepDependsOn,
    setStepInput,
    setSelectedStepId,
    setStepAction,
    setStepRemoteTaskId,
    setStepOutput,
    setStepFailText,
    setStepInputReason,
    setStepInputRequiredPayload,
    setStepResumePayload,
    dispatch,
  };
}
