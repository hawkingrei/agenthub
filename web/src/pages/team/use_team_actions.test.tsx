// @vitest-environment jsdom
import React, { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamStepRecord,
} from "../../api";
import { useTeamActions } from "./use_team_actions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type TeamActions = ReturnType<typeof useTeamActions>;
type TeamActionsInput = Parameters<typeof useTeamActions>[0];

type HookHarnessProps = {
  options: TeamActionsInput;
  onCapture: (actions: TeamActions) => void;
};

function HookHarness(props: HookHarnessProps) {
  const { options, onCapture } = props;
  const actions = useTeamActions(options);
  useEffect(() => {
    onCapture(actions);
  }, [actions, onCapture]);
  return null;
}

function createBaseOptions(overrides: Partial<TeamActionsInput> = {}): TeamActionsInput {
  const options: TeamActionsInput = {
    token: "token-1",
    selectedTeamId: "team-1",
    runContextId: "",
    runInput: "{}",
    runLookupId: "",
    runStatusFilter: "all",
    runsLoading: false,
    runsHasMore: false,
    runsBeforeCreatedAt: undefined,
    selectedStepId: "",
    activeRunIdForSelectedTeam: null,
    activeRunForSelectedTeam: null,
    inboxActorId: "",
    inboxLimit: "100",
    inboxAfterId: "",
    inboxIncludeDelivered: false,
    selectedMemberSnapshot: null,
    activeRunIdRef: { current: null },
    eventsRef: { current: [] as TeamRunEventRecord[] },
    memberEventsRef: { current: [] as AgentEvent[] },
    setBusy: vi.fn(),
    setError: vi.fn(),
    setAgents: vi.fn(),
    setTeams: vi.fn(),
    setSelectedTeamId: vi.fn(),
    setRuns: vi.fn(),
    setTeamRunBrowserByTeam: vi.fn(),
    setRunsLoading: vi.fn(),
    setSteps: vi.fn(),
    setSelectedStepId: vi.fn(),
    setEvents: vi.fn(),
    setEventsLoading: vi.fn(),
    setEventsHasMore: vi.fn(),
    setSnapshot: vi.fn(),
    setSnapshotLoading: vi.fn(),
    setInbox: vi.fn<(next: TeamActorMessageRecord[]) => void>(),
    setMemberEvents: vi.fn(),
    setMemberEventsLoading: vi.fn(),
    setMemberEventsHasMore: vi.fn(),
    setActiveRunId: vi.fn(),
    setRunLookupId: vi.fn(),
  };
  return { ...options, ...overrides };
}

async function mountHarness(
  options: TeamActionsInput,
  onCapture: (actions: TeamActions) => void
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

describe("useTeamActions", () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it("keeps API action callbacks stable when inputs are unchanged", async () => {
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const baseOptions = createBaseOptions();

    const { root, container } = await mountHarness(baseOptions, onCapture);
    try {
      const initial = captures[captures.length - 1];
      expect(initial).toBeDefined();

      const sameValueOptions = { ...baseOptions };
      await act(async () => {
        root.render(<HookHarness options={sameValueOptions} onCapture={onCapture} />);
        await Promise.resolve();
      });

      const rerendered = captures[captures.length - 1];
      expect(rerendered).toBeDefined();
      expect(rerendered.refreshRun).toBe(initial.refreshRun);
      expect(rerendered.refreshEvents).toBe(initial.refreshEvents);
      expect(rerendered.refreshSnapshot).toBe(initial.refreshSnapshot);
      expect(rerendered.onCreateRun).toBe(initial.onCreateRun);
      expect(rerendered.onLoadRunById).toBe(initial.onLoadRunById);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("keeps critical lifecycle callbacks stable when non-token inputs change", async () => {
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const baseOptions = createBaseOptions({
      selectedTeamId: "team-1",
      activeRunIdForSelectedTeam: "run-1",
      runsHasMore: true,
      runsBeforeCreatedAt: 111,
      selectedStepId: "step-1",
      inboxActorId: "leader-1",
      inboxLimit: "100",
      inboxAfterId: "10",
      inboxIncludeDelivered: false,
    });

    const { root, container } = await mountHarness(baseOptions, onCapture);
    try {
      const initial = captures[captures.length - 1];
      expect(initial).toBeDefined();

      const nextOptions = {
        ...baseOptions,
        selectedStepId: "step-2",
        inboxLimit: "200",
        inboxAfterId: "20",
        inboxIncludeDelivered: true,
        runsLoading: true,
        runsHasMore: false,
        runsBeforeCreatedAt: 222,
        runStatusFilter: "working" as const,
      };
      await act(async () => {
        root.render(<HookHarness options={nextOptions} onCapture={onCapture} />);
        await Promise.resolve();
      });

      const rerendered = captures[captures.length - 1];
      expect(rerendered).toBeDefined();
      expect(rerendered.refreshSteps).toBe(initial.refreshSteps);
      expect(rerendered.loadInbox).toBe(initial.loadInbox);
      expect(rerendered.onLoadMoreRuns).toBe(initial.onLoadMoreRuns);
    } finally {
      cleanupHarness(root, container);
    }
  });

  it("rebuilds API callbacks only when token changes", async () => {
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const baseOptions = createBaseOptions();

    const { root, container } = await mountHarness(baseOptions, onCapture);
    try {
      const initial = captures[captures.length - 1];
      expect(initial).toBeDefined();

      const nextOptions = createBaseOptions({
        token: "token-2",
        activeRunForSelectedTeam: {
          id: "run-1",
          team_id: "team-1",
          context_id: "ctx-1",
          status: "working",
          input: {},
          created_at: 1,
          started_at: null,
          ended_at: null,
        } as TeamRunRecord,
        selectedMemberSnapshot: {
          member_id: "member-1",
          role: "worker",
          model: null,
          prompt: null,
          skills: [],
          pending_inbox_count: 0,
          status: "idle",
          latest_step: {
            id: "step-1",
            run_id: "run-1",
            step_key: "analysis",
            member_id: "member-1",
            status: "working",
            attempt: 1,
            depends_on: [],
          } as TeamStepRecord,
          session_status: null,
        },
        activeRunIdRef: baseOptions.activeRunIdRef,
        eventsRef: baseOptions.eventsRef,
        memberEventsRef: baseOptions.memberEventsRef,
        setBusy: baseOptions.setBusy,
        setError: baseOptions.setError,
        setAgents: baseOptions.setAgents,
        setTeams: baseOptions.setTeams,
        setSelectedTeamId: baseOptions.setSelectedTeamId,
        setRuns: baseOptions.setRuns,
        setTeamRunBrowserByTeam: baseOptions.setTeamRunBrowserByTeam,
        setRunsLoading: baseOptions.setRunsLoading,
        setSteps: baseOptions.setSteps,
        setSelectedStepId: baseOptions.setSelectedStepId,
        setEvents: baseOptions.setEvents,
        setEventsLoading: baseOptions.setEventsLoading,
        setEventsHasMore: baseOptions.setEventsHasMore,
        setSnapshot: baseOptions.setSnapshot,
        setSnapshotLoading: baseOptions.setSnapshotLoading,
        setInbox: baseOptions.setInbox,
        setMemberEvents: baseOptions.setMemberEvents,
        setMemberEventsLoading: baseOptions.setMemberEventsLoading,
        setMemberEventsHasMore: baseOptions.setMemberEventsHasMore,
        setActiveRunId: baseOptions.setActiveRunId,
        setRunLookupId: baseOptions.setRunLookupId,
      });

      await act(async () => {
        root.render(<HookHarness options={nextOptions} onCapture={onCapture} />);
        await Promise.resolve();
      });

      const rerendered = captures[captures.length - 1];
      expect(rerendered).toBeDefined();
      expect(rerendered.refreshRun).not.toBe(initial.refreshRun);
      expect(rerendered.refreshEvents).not.toBe(initial.refreshEvents);
      expect(rerendered.refreshSnapshot).not.toBe(initial.refreshSnapshot);
      expect(rerendered.onCreateRun).not.toBe(initial.onCreateRun);
    } finally {
      cleanupHarness(root, container);
    }
  });
});
