// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, type TeamStepRecord } from "../../api";
import { useTeamStepActions } from "./use_team_step_actions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type HookParams = Parameters<typeof useTeamStepActions>[0];
type HookSnapshot = ReturnType<typeof useTeamStepActions>;

function makeStep(id: string, runId: string): TeamStepRecord {
  return {
    id,
    run_id: runId,
    step_key: "step-key",
    member_id: "worker-1",
    remote_task_id: null,
    status: "submitted",
    attempt: 0,
    depends_on: [],
    input: {},
    output: null,
    error_text: null,
    started_at: null,
    ended_at: null,
  };
}

function createParams(overrides: Partial<HookParams> = {}): HookParams {
  return {
    token: "token-1",
    activeRunId: "run-1",
    stepKey: "step-main",
    stepMemberId: "worker-1",
    stepDependsOn: "a, b, , c",
    stepInput: '{"plan": 1}',
    selectedStepId: "step-1",
    stepAction: "start",
    stepRemoteTaskId: " remote-1 ",
    stepOutput: '{"ok": true}',
    stepFailText: "failure text",
    stepInputReason: "need-input",
    stepInputRequiredPayload: '{"needed":"value"}',
    stepResumePayload: '{"resume":"value"}',
    setBusy: vi.fn(),
    setError: vi.fn(),
    parseErrorMessage: vi.fn(() => "parsed-error"),
    refreshRun: vi.fn().mockResolvedValue(undefined),
    refreshSteps: vi.fn().mockResolvedValue(undefined),
    refreshEvents: vi.fn().mockResolvedValue(undefined),
    refreshSnapshot: vi.fn().mockResolvedValue(undefined),
    setSelectedStepId: vi.fn(),
    ...overrides,
  };
}

function HookHarness({
  params,
  onSnapshot,
}: {
  params: HookParams;
  onSnapshot: (snapshot: HookSnapshot) => void;
}) {
  const snapshot = useTeamStepActions(params);
  onSnapshot(snapshot);
  return null;
}

describe("useTeamStepActions", () => {
  let container: HTMLDivElement;
  let root: Root;
  let snapshot: HookSnapshot | null = null;

  beforeEach(() => {
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    snapshot = null;
    vi.restoreAllMocks();
  });

  it("validates submit preconditions before calling API", async () => {
    const paramsNoRun = createParams({ activeRunId: null });
    act(() => {
      root.render(
        <HookHarness
          params={paramsNoRun}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });
    await act(async () => {
      await snapshot?.onSubmitStep();
    });
    expect(paramsNoRun.setError).toHaveBeenCalledWith("Select a run first");

    const paramsNoStepKey = createParams({ stepKey: "   " });
    act(() => {
      root.render(
        <HookHarness
          params={paramsNoStepKey}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });
    await act(async () => {
      await snapshot?.onSubmitStep();
    });
    expect(paramsNoStepKey.setError).toHaveBeenCalledWith("step_key is required");

    const paramsNoMember = createParams({ stepMemberId: "   " });
    act(() => {
      root.render(
        <HookHarness
          params={paramsNoMember}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });
    await act(async () => {
      await snapshot?.onSubmitStep();
    });
    expect(paramsNoMember.setError).toHaveBeenCalledWith("member_id is required");
  });

  it("submits step, parses input/depends_on, and refreshes run views", async () => {
    const params = createParams();
    vi.spyOn(api, "submitTeamRunStep").mockResolvedValue(
      makeStep("step-created", "run-1")
    );

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onSubmitStep();
    });

    expect(api.submitTeamRunStep).toHaveBeenCalledWith("token-1", "run-1", {
      step_key: "step-main",
      member_id: "worker-1",
      depends_on: ["a", "b", "c"],
      input: { plan: 1 },
    });
    expect(params.refreshRun).toHaveBeenCalledWith("run-1");
    expect(params.refreshSteps).toHaveBeenCalledWith("run-1");
    expect(params.refreshEvents).toHaveBeenCalledWith("run-1");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
    expect(params.setSelectedStepId).toHaveBeenCalledWith("step-created");
  });

  it("applies start action with trimmed remote task id", async () => {
    const params = createParams({
      stepAction: "start",
      stepRemoteTaskId: "  remote-7  ",
    });
    vi.spyOn(api, "startTeamRunStep").mockResolvedValue(makeStep("step-1", "run-1"));

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onApplyStepAction();
    });

    expect(api.startTeamRunStep).toHaveBeenCalledWith("token-1", "run-1", "step-1", {
      remote_task_id: "remote-7",
    });
    expect(params.refreshRun).toHaveBeenCalledWith("run-1");
    expect(params.refreshSteps).toHaveBeenCalledWith("run-1");
    expect(params.refreshEvents).toHaveBeenCalledWith("run-1");
    expect(params.refreshSnapshot).toHaveBeenCalledWith("run-1");
  });

  it("rejects fail action without failure reason", async () => {
    const params = createParams({
      stepAction: "fail",
      stepFailText: "   ",
      parseErrorMessage: vi.fn(() => "friendly-fail-error"),
    });
    const failSpy = vi.spyOn(api, "failTeamRunStep");

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onApplyStepAction();
    });

    expect(failSpy).not.toHaveBeenCalled();
    expect(params.setError).toHaveBeenCalledWith("friendly-fail-error");
  });

  it("applies input_required action and normalizes optional reason", async () => {
    const params = createParams({
      stepAction: "input_required",
      stepInputReason: "   ",
      stepInputRequiredPayload: '{"reason":"need-details"}',
    });
    vi.spyOn(api, "setTeamRunStepInputRequired").mockResolvedValue(
      makeStep("step-1", "run-1")
    );

    act(() => {
      root.render(
        <HookHarness
          params={params}
          onSnapshot={(next) => {
            snapshot = next;
          }}
        />
      );
    });

    await act(async () => {
      await snapshot?.onApplyStepAction();
    });

    expect(api.setTeamRunStepInputRequired).toHaveBeenCalledWith(
      "token-1",
      "run-1",
      "step-1",
      {
        reason: undefined,
        input: { reason: "need-details" },
      }
    );
  });
});
