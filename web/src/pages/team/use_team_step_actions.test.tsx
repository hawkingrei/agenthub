// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { type TeamRunRecord, type TeamRunSnapshotRecord, api } from "../../api";
import type { StepAction } from "./state";
import { useTeamStepActions } from "./use_team_step_actions";

vi.mock("../../api", () => ({
  api: {
    submitTeamRunStep: vi.fn(),
    startTeamRunStep: vi.fn(),
    completeTeamRunStep: vi.fn(),
    failTeamRunStep: vi.fn(),
    setTeamRunStepInputRequired: vi.fn(),
    resumeTeamRunStep: vi.fn(),
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type TeamStepActions = ReturnType<typeof useTeamStepActions>;
type TeamStepActionsInput = Parameters<typeof useTeamStepActions>[0];

type HookHarnessProps = {
  options: TeamStepActionsInput;
  onCapture: (actions: TeamStepActions) => void;
};

function HookHarness(props: HookHarnessProps) {
  const { options, onCapture } = props;
  const actions = useTeamStepActions(options);
  useEffect(() => {
    onCapture(actions);
  }, [actions, onCapture]);
  return null;
}

function createBaseOptions(
  overrides: Partial<TeamStepActionsInput> = {}
): TeamStepActionsInput {
  const options: TeamStepActionsInput = {
    token: "token-1",
    activeRunIdForSelectedTeam: "run-1",
    selectedStepId: "step-1",
    stepAction: "complete",
    stepKey: "leader_plan",
    stepMemberId: "leader-1",
    stepDependsOn: "worker_a,worker_b",
    stepInput: '{"task":"investigate"}',
    stepRemoteTaskId: "",
    stepOutput: '{"status":"done"}',
    stepFailText: "",
    stepInputReason: "",
    stepInputRequiredPayload: "{}",
    stepResumePayload: "{}",
    setBusy: vi.fn(),
    setError: vi.fn(),
    setSelectedStepId: vi.fn(),
    refreshRun: vi.fn(async () => {
      return {
        id: "run-1",
        team_id: "team-1",
        context_id: "ctx-1",
        status: "working",
        input: {},
        created_at: 1,
        started_at: null,
        ended_at: null,
      } as TeamRunRecord;
    }),
    refreshSteps: vi.fn(async () => undefined),
    refreshEvents: vi.fn(async () => undefined),
    refreshSnapshot: vi.fn(async () => {
      return {
        run: {
          id: "run-1",
          team_id: "team-1",
          context_id: "ctx-1",
          status: "working",
          input: {},
          created_at: 1,
          started_at: null,
          ended_at: null,
        },
        team: {
          id: "team-1",
          name: "Team One",
          description: null,
          spec: {},
          created_at: 1,
          updated_at: 1,
        },
        leader_member_id: "leader-1",
        members: [],
        steps: [],
        latest_events: [],
        mailbox: {
          pending: 0,
          delivered: 0,
          dead_letter: 0,
          recent_messages: [],
        },
      } as TeamRunSnapshotRecord;
    }),
  };
  return { ...options, ...overrides };
}

async function mountHarness(
  options: TeamStepActionsInput,
  onCapture: (actions: TeamStepActions) => void
): Promise<{ root: Root; container: HTMLDivElement }> {
  const container = document.createElement("div");
  document.body.appendChild(container);
  const root = createRoot(container);
  await act(async () => {
    root.render(<HookHarness options={options} onCapture={onCapture} />);
    await Promise.resolve();
  });
  return { root, container };
}

function cleanupHarness(root: Root, container: HTMLDivElement): void {
  act(() => {
    root.unmount();
  });
  container.remove();
}

describe("useTeamStepActions", () => {
  const mockedApi = vi.mocked(api);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("submits step payload and refreshes run views", async () => {
    mockedApi.submitTeamRunStep.mockResolvedValueOnce({
      id: "step-created-1",
      run_id: "run-1",
      step_key: "leader_plan",
      member_id: "leader-1",
      status: "submitted",
      attempt: 1,
      depends_on: [],
    } as Awaited<ReturnType<typeof api.submitTeamRunStep>>);

    let captured: TeamStepActions | null = null;
    const options = createBaseOptions({
      stepDependsOn: "worker_a, worker_b , , worker_c",
      stepInput: '{"payload":1}',
    });
    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      expect(captured).not.toBeNull();
      await act(async () => {
        await captured?.onSubmitStep();
      });

      expect(mockedApi.submitTeamRunStep).toHaveBeenCalledWith(
        "token-1",
        "run-1",
        {
          step_key: "leader_plan",
          member_id: "leader-1",
          depends_on: ["worker_a", "worker_b", "worker_c"],
          input: { payload: 1 },
        }
      );
      expect(options.refreshRun).toHaveBeenCalledWith("run-1");
      expect(options.refreshSteps).toHaveBeenCalledWith("run-1");
      expect(options.refreshEvents).toHaveBeenCalledWith("run-1");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
      expect(options.setSelectedStepId).toHaveBeenCalledWith("step-created-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("applies complete action with parsed output payload", async () => {
    mockedApi.completeTeamRunStep.mockResolvedValueOnce({
      id: "step-1",
      run_id: "run-1",
      step_key: "leader_plan",
      member_id: "leader-1",
      status: "completed",
      attempt: 1,
      depends_on: [],
      output: { status: "done" },
    } as Awaited<ReturnType<typeof api.completeTeamRunStep>>);

    let captured: TeamStepActions | null = null;
    const options = createBaseOptions({
      stepAction: "complete" as StepAction,
      stepOutput: '{"status":"done","count":2}',
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      expect(captured).not.toBeNull();
      await act(async () => {
        await captured?.onApplyStepAction();
      });

      expect(mockedApi.completeTeamRunStep).toHaveBeenCalledWith(
        "token-1",
        "run-1",
        "step-1",
        {
          output: { status: "done", count: 2 },
        }
      );
      expect(options.refreshRun).toHaveBeenCalledWith("run-1");
      expect(options.refreshSteps).toHaveBeenCalledWith("run-1");
      expect(options.refreshEvents).toHaveBeenCalledWith("run-1");
      expect(options.refreshSnapshot).toHaveBeenCalledWith("run-1");
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("returns validation error when fail action has empty reason", async () => {
    let captured: TeamStepActions | null = null;
    const options = createBaseOptions({
      stepAction: "fail" as StepAction,
      stepFailText: "   ",
    });

    const { root, container } = await mountHarness(options, (actions) => {
      captured = actions;
    });

    try {
      expect(captured).not.toBeNull();
      await act(async () => {
        await captured?.onApplyStepAction();
      });

      expect(mockedApi.failTeamRunStep).not.toHaveBeenCalled();
      expect(options.setError).toHaveBeenCalledWith("Fail reason is required");
    } finally {
      cleanupHarness(root, container);
    }
  });
});
