// @vitest-environment jsdom
import { act, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamStepRecord,
} from "../../api";
import { api } from "../../api";
import { saveTeamMemberAcpRenderCache, clearTeamMemberAcpRenderCache } from "./team_member_acp_render_cache";
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
    inboxLimit: "50",
    inboxAfterId: "",
    inboxIncludeDelivered: false,
    selectedMemberAgentId: null,
    selectedMemberSessionId: null,
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
    clearTeamMemberAcpRenderCache();
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
      inboxLimit: "50",
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

  it("loads member events from runtime session fallback without a run snapshot", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents").mockResolvedValueOnce([
      {
        event_id: 7,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "7",
        ts: 123,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text: "Can you summarize the failure?",
        }),
      },
      {
        event_id: 8,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "8",
        ts: 124,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "I found the failing step and the root cause.",
        }),
      },
    ]);
    const setMemberEvents = vi.fn();
    const setMemberEventsHasMore = vi.fn();
    const setMemberEventsLoading = vi.fn();
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      setMemberEvents,
      setMemberEventsHasMore,
      setMemberEventsLoading,
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenCalledWith(
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
      expect(setMemberEventsLoading).toHaveBeenCalledWith(true);
      expect(setMemberEventsHasMore).toHaveBeenCalledWith(true);
      expect(setMemberEvents).toHaveBeenCalled();
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("coalesces concurrent replace loads for the same member ACP session", async () => {
    let resolveList: ((events: AgentEvent[]) => void) | null = null;
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockImplementationOnce(
        () =>
          new Promise<AgentEvent[]>((resolve) => {
            resolveList = resolve;
          })
      );
    const setMemberEvents = vi.fn();
    const setMemberEventsHasMore = vi.fn();
    const setMemberEventsLoading = vi.fn();
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      setMemberEvents,
      setMemberEventsHasMore,
      setMemberEventsLoading,
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();

      let firstPromise: Promise<void> | null = null;
      let secondPromise: Promise<void> | null = null;
      await act(async () => {
        firstPromise = actions.loadMemberEvents("replace");
        secondPromise = actions.loadMemberEvents("replace");
        await Promise.resolve();
      });

      expect(listAgentEvents).toHaveBeenCalledTimes(1);
      expect(setMemberEventsLoading).toHaveBeenCalledTimes(1);

      await act(async () => {
        resolveList?.([
          {
            event_id: 11,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "11",
            ts: 123,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "Latest visible content.",
            }),
          },
        ]);
        await Promise.all([firstPromise, secondPromise]);
      });

      expect(setMemberEvents).toHaveBeenCalledTimes(1);
      expect(setMemberEventsHasMore).toHaveBeenCalledWith(true);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not auto-prefetch older ACP history when the first page already renders a partial chunked message", async () => {
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce([
        {
          event_id: 11,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "11",
          ts: 123,
          stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "tail chunk",
          chunk: true,
          message_id: "message-0",
          chunk_index: 2,
        }),
      },
      ])
      .mockResolvedValueOnce([
        {
          event_id: 9,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "9",
          ts: 121,
          stream: "acp",
          message: JSON.stringify({
            type: "user_message",
            text: "What happened?",
          }),
        },
        {
          event_id: 10,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "10",
          ts: 122,
          stream: "acp",
          message: JSON.stringify({
            type: "agent_message",
            text: "head chunk",
            chunk: true,
            message_id: "message-0",
            chunk_index: 0,
          }),
        },
      ]);
    const setMemberEvents = vi.fn();
    const setMemberEventsHasMore = vi.fn();
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      setMemberEvents,
      setMemberEventsHasMore,
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
      const update = setMemberEvents.mock.calls[0]?.[0];
      expect(typeof update).toBe("function");
      expect(update([]).map((event: AgentEvent) => event.event_id)).toEqual([11]);
      expect(setMemberEventsHasMore).toHaveBeenCalledWith(true);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not keep prefetching when complete visible messages already exist after omitting an incomplete leading chunk", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents").mockResolvedValueOnce([
      {
        event_id: 20,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "20",
        ts: 124,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text: "continue",
        }),
      },
      {
        event_id: 21,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "21",
        ts: 125,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "tail chunk",
          chunk: true,
          message_id: "message-1",
          chunk_index: 4,
        }),
      },
    ]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not auto-prefetch even for very long chunked messages without warm visible content", async () => {
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce([
        {
          event_id: 100,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "100",
          ts: 123,
          stream: "acp",
          message: JSON.stringify({
            type: "agent_message",
            text: "tail chunk",
            chunk: true,
            message_id: "message-1",
            chunk_index: 320,
          }),
        },
      ]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not auto-prefetch when the first page is dominated by one chunked message", async () => {
    const firstPage = Array.from({ length: 12 }, (_, index) => ({
      event_id: 200 + index,
      agent_id: "worker-agent",
      session_id: "runtime-session-1",
      seq: String(200 + index),
      ts: 200 + index,
      stream: "acp" as const,
      message: JSON.stringify({
        type: "agent_message",
        text: `chunk-${index}`,
        chunk: true,
        message_id: "message-2",
        chunk_index: 40 + index,
      }),
    }));
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce(firstPage);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenNthCalledWith(
        1,
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not prefetch older ACP history on replace when warm render cache already has visible content", async () => {
    saveTeamMemberAcpRenderCache("worker-agent", "runtime-session-1", [
      {
        event_id: 1,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "1",
        ts: 100,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "Cached visible message.",
        }),
      },
    ]);
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce([
        {
          event_id: 11,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "11",
          ts: 123,
          stream: "acp",
          message: JSON.stringify({
            type: "agent_message",
            text: "tail chunk",
            chunk: true,
            message_id: "message-0",
            chunk_index: 2,
          }),
        },
      ]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
      expect(listAgentEvents).toHaveBeenCalledWith(
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("does not prefetch older ACP history on replace when current session state already has visible content", async () => {
    const listAgentEvents = vi
      .spyOn(api, "listAgentEvents")
      .mockResolvedValueOnce([
        {
          event_id: 11,
          agent_id: "worker-agent",
          session_id: "runtime-session-1",
          seq: "11",
          ts: 123,
          stream: "acp",
          message: JSON.stringify({
            type: "agent_message",
            text: "tail chunk",
            chunk: true,
            message_id: "message-0",
            chunk_index: 2,
          }),
        },
      ]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      memberEventsRef: {
        current: [
          {
            event_id: 1,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "1",
            ts: 100,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "Hydrated visible content.",
            }),
          },
        ],
      },
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenCalledTimes(1);
      expect(listAgentEvents).toHaveBeenCalledWith(
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        undefined
      );
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("prefers the resolved member agent id when loading detached ACP events", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents").mockResolvedValueOnce([
      {
        event_id: 9,
        agent_id: "agent-123",
        session_id: "runtime-session-1",
        seq: "9",
        ts: 123,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text: "Show me the latest progress.",
        }),
      },
      {
        event_id: 10,
        agent_id: "agent-123",
        session_id: "runtime-session-1",
        seq: "10",
        ts: 124,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "Latest progress is now available.",
        }),
      },
    ]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "agent-123",
      selectedMemberSessionId: "runtime-session-1",
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).toHaveBeenCalledWith(
        "token-1",
        "agent-123",
        60,
        "runtime-session-1",
        undefined
      );
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("clears detached ACP events when the resolved member agent id is missing", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents");
    const setMemberEvents = vi.fn();
    const setMemberEventsHasMore = vi.fn();
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: null,
      selectedMemberSessionId: "runtime-session-1",
      setMemberEvents,
      setMemberEventsHasMore,
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      expect(listAgentEvents).not.toHaveBeenCalled();
      expect(setMemberEvents).toHaveBeenCalledWith([]);
      expect(setMemberEventsHasMore).toHaveBeenCalledWith(false);
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("keeps older same-session ACP history on replace refresh", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents").mockResolvedValueOnce([
      {
        event_id: 7,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "7",
        ts: 123,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text: "What changed since the previous attempt?",
        }),
      },
      {
        event_id: 8,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "8",
        ts: 124,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "I refreshed the latest runtime output.",
        }),
      },
    ]);
    const setMemberEvents = vi.fn();
    const setMemberEventsHasMore = vi.fn();
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const existingHistory: AgentEvent[] = [
      {
        event_id: 1,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "1",
        ts: 100,
        stream: "acp",
        message: "old-1",
      },
      {
        event_id: 2,
        agent_id: "worker-agent",
        session_id: "runtime-session-1",
        seq: "2",
        ts: 101,
        stream: "acp",
        message: "old-2",
      },
    ];
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      memberEventsRef: { current: existingHistory },
      setMemberEvents,
      setMemberEventsHasMore,
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("replace");
      });
      const update = setMemberEvents.mock.calls[0]?.[0];
      expect(typeof update).toBe("function");
      expect(update(existingHistory).map((event: AgentEvent) => event.event_id)).toEqual([
        1, 2, 7, 8,
      ]);
      expect(setMemberEventsHasMore).not.toHaveBeenCalled();
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });

  it("uses the active session head when loading older ACP history", async () => {
    const listAgentEvents = vi.spyOn(api, "listAgentEvents").mockResolvedValueOnce([]);
    const captures: TeamActions[] = [];
    const onCapture = (actions: TeamActions) => {
      captures.push(actions);
    };
    const options = createBaseOptions({
      selectedMemberAgentId: "worker-agent",
      selectedMemberSessionId: "runtime-session-1",
      memberEventsRef: {
        current: [
          {
            event_id: 100,
            agent_id: "worker-agent",
            session_id: "runtime-session-2",
            seq: "100",
            ts: 200,
            stream: "acp",
            message: "other-session",
          },
          {
            event_id: 10,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "10",
            ts: 100,
            stream: "acp",
            message: "current-session",
          },
        ],
      },
    });

    const { root, container } = await mountHarness(options, onCapture);
    try {
      const actions = captures[captures.length - 1];
      expect(actions).toBeDefined();
      await act(async () => {
        await actions.loadMemberEvents("prepend");
      });
      expect(listAgentEvents).toHaveBeenCalledWith(
        "token-1",
        "worker-agent",
        60,
        "runtime-session-1",
        10
      );
    } finally {
      listAgentEvents.mockRestore();
      cleanupHarness(root, container);
    }
  });
});
