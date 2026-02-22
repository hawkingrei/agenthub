// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  AgentEvent,
  TeamActorMessageRecord,
  TeamDefinitionRecord,
  TeamMemberSnapshot,
  TeamRunEventRecord,
  TeamRunRecord,
  TeamRunSnapshotRecord,
  TeamStepRecord,
} from "../api";
import { TeamEventsPanel } from "./team_events_panel";
import { TeamMailboxPanel } from "./team_mailbox_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function required<T>(value: T | null | undefined, message: string): T {
  if (value == null) {
    throw new Error(message);
  }
  return value;
}

function clickElement(element: Element | null): void {
  const node = required(element, "element not found");
  act(() => {
    node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

function findButtonByText(container: HTMLElement, text: string): HTMLButtonElement {
  const button = Array.from(container.querySelectorAll("button")).find((candidate) =>
    candidate.textContent?.includes(text)
  ) as HTMLButtonElement | undefined;
  return required(button, `button not found: ${text}`);
}

function findButtonByAriaLabel(container: HTMLElement, label: string): HTMLButtonElement {
  const normalized = label.toLowerCase();
  const button = Array.from(container.querySelectorAll("button")).find((candidate) =>
    candidate.getAttribute("aria-label")?.toLowerCase().includes(normalized)
  ) as HTMLButtonElement | undefined;
  return required(button, `button not found by aria-label: ${label}`);
}

function setNativeValue(
  element: HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement,
  value: string
): void {
  const prototype = Object.getPrototypeOf(element) as {
    value?: unknown;
  };
  const descriptor = Object.getOwnPropertyDescriptor(prototype, "value");
  if (descriptor?.set) {
    descriptor.set.call(element, value);
    return;
  }
  element.value = value;
}

function changeInputValue(
  element: HTMLInputElement | HTMLTextAreaElement,
  value: string
): void {
  act(() => {
    setNativeValue(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

function changeSelectValue(element: HTMLSelectElement, value: string): void {
  act(() => {
    setNativeValue(element, value);
    element.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

function toggleCheckboxValue(element: HTMLInputElement, checked: boolean): void {
  act(() => {
    if (element.checked !== checked) {
      element.click();
    }
  });
}

function buildTeam(overrides: Partial<TeamDefinitionRecord> = {}): TeamDefinitionRecord {
  return {
    id: "team-1",
    name: "Team One",
    description: null,
    spec: {},
    created_at: 1,
    updated_at: 2,
    ...overrides,
  };
}

function buildRun(overrides: Partial<TeamRunRecord> = {}): TeamRunRecord {
  return {
    id: "run-1",
    team_id: "team-1",
    context_id: "ctx-1",
    status: "working",
    input: {},
    created_at: 100,
    started_at: 110,
    ended_at: null,
    ...overrides,
  };
}

function buildStep(overrides: Partial<TeamStepRecord> = {}): TeamStepRecord {
  return {
    id: "step-1",
    run_id: "run-1",
    step_key: "plan",
    member_id: "leader-agent",
    remote_task_id: null,
    status: "working",
    attempt: 1,
    depends_on: ["seed"],
    input: {},
    output: {},
    error_text: null,
    started_at: 100,
    ended_at: null,
    ...overrides,
  };
}

function buildRunEvent(eventId: number, payload: unknown = {}): TeamRunEventRecord {
  return {
    event_id: eventId,
    run_id: "run-1",
    step_id: null,
    event_type: "agent_message",
    ts: 1_700_000_000 + eventId,
    payload,
  };
}

function buildMailboxMessage(
  messageId: number,
  overrides: Partial<TeamActorMessageRecord> = {}
): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: "leader-agent",
    to_actor_id: "worker-agent",
    channel: "default",
    transport: "local",
    route: null,
    payload: { type: "chat_message", text: `msg-${messageId}` },
    status: "pending",
    created_at: 1_700_000_000 + messageId,
    delivered_at: null,
    ...overrides,
  };
}

function buildMemberSnapshot(overrides: Partial<TeamMemberSnapshot> = {}): TeamMemberSnapshot {
  return {
    member_id: "leader-agent",
    role: "leader",
    model: "gpt-5",
    prompt: "plan",
    skills: ["team-leader-orchestrator"],
    pending_inbox_count: 1,
    status: "working",
    latest_step: buildStep(),
    session_status: "active",
    ...overrides,
  };
}

function buildSnapshot(overrides: Partial<TeamRunSnapshotRecord> = {}): TeamRunSnapshotRecord {
  return {
    run: buildRun(),
    team: buildTeam(),
    leader_member_id: "leader-agent",
    members: [
      buildMemberSnapshot(),
      buildMemberSnapshot({
        member_id: "worker-agent",
        role: "worker",
        model: null,
        prompt: null,
        skills: ["team-worker-executor"],
        pending_inbox_count: 3,
      }),
    ],
    steps: [buildStep()],
    latest_events: [buildRunEvent(1, { text: "event" })],
    mailbox: {
      pending: 3,
      delivered: 2,
      dead_letter: 1,
      recent_messages: [buildMailboxMessage(1)],
    },
    ...overrides,
  };
}

describe("team panels interactions", () => {
  let container: HTMLDivElement;
  let root: Root;

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
  });

  it("TeamSidebar renders summary and triggers refresh/create/select callbacks", () => {
    const onRefreshTeams = vi.fn();
    const onOpenCreateTeamWizard = vi.fn();
    const onOpenCreateTeamManual = vi.fn();
    const onSelectTeam = vi.fn();

    act(() => {
      root.render(
        <TeamSidebar
          busy={null}
          onRefreshTeams={onRefreshTeams}
          onOpenCreateTeamWizard={onOpenCreateTeamWizard}
          onOpenCreateTeamManual={onOpenCreateTeamManual}
          draftTeamName="alpha"
          leaderMemberId="leader-agent"
          configuredWorkerCount={2}
          teams={[buildTeam()]}
          selectedTeamId={null}
          teamMemberSummaryByTeamId={new Map([
            [
              "team-1",
              {
                active: 1,
                inactive: 1,
                missing: 0,
                total: 2,
              },
            ],
          ])}
          onSelectTeam={onSelectTeam}
        />
      );
    });

    clickElement(findButtonByAriaLabel(container, "Refresh teams"));
    clickElement(findButtonByText(container, "Guided Wizard"));
    clickElement(findButtonByText(container, "Manual Spec"));
    clickElement(required(container.querySelector(".teams-list .team-item"), "team item missing"));

    expect(onRefreshTeams).toHaveBeenCalledTimes(1);
    expect(onOpenCreateTeamWizard).toHaveBeenCalledTimes(1);
    expect(onOpenCreateTeamManual).toHaveBeenCalledTimes(1);
    expect(onSelectTeam).toHaveBeenCalledWith("team-1");
    expect(container.textContent).toContain("active=1 inactive=1 missing=0 total=2");

    act(() => {
      root.render(
        <TeamSidebar
          busy={null}
          onRefreshTeams={() => {}}
          onOpenCreateTeamWizard={() => {}}
          onOpenCreateTeamManual={() => {}}
          draftTeamName=""
          leaderMemberId=""
          configuredWorkerCount={0}
          teams={[]}
          selectedTeamId={null}
          teamMemberSummaryByTeamId={new Map()}
          onSelectTeam={() => {}}
        />
      );
    });

    expect(container.textContent).toContain("No teams yet.");
  });

  it("TeamRunPanel supports run filter/list interactions and empty-state messages", () => {
    const onDeleteTeam = vi.fn();
    const onRunStatusFilterChange = vi.fn();
    const onRefreshRuns = vi.fn();
    const onActiveRunChange = vi.fn();
    const onLoadMoreRuns = vi.fn();

    const activeRun = buildRun({ id: "run-1" });

    act(() => {
      root.render(
        <TeamRunPanel
          selectedTeam={buildTeam()}
          busy={null}
          onDeleteTeam={onDeleteTeam}
          runStatusFilter="all"
          runStatusFilterOptions={[
            { value: "all", label: "All" },
            { value: "working", label: "Working" },
          ]}
          onRunStatusFilterChange={onRunStatusFilterChange}
          onRefreshRuns={onRefreshRuns}
          runsLoading={false}
          visibleRuns={[activeRun]}
          activeRunId={null}
          onActiveRunChange={onActiveRunChange}
          isActiveRunHiddenByFilter={false}
          activeRun={activeRun}
          totalLoadedRunsForTeam={1}
          pageLimit={20}
          runsHasMore={true}
          selectedTeamId="team-1"
          onLoadMoreRuns={onLoadMoreRuns}
        />
      );
    });

    changeSelectValue(
      required(
        container.querySelector('select[aria-label="Run status filter"]') as HTMLSelectElement | null,
        "run filter select missing"
      ),
      "working"
    );

    clickElement(findButtonByText(container, "Delete Team"));
    clickElement(findButtonByAriaLabel(container, "Refresh runs"));
    clickElement(required(container.querySelector(".teams-run-list .team-item"), "run list item missing"));
    clickElement(findButtonByText(container, "Load More"));

    expect(onDeleteTeam).toHaveBeenCalledTimes(1);
    expect(onRunStatusFilterChange).toHaveBeenCalledWith("working");
    expect(onRefreshRuns).toHaveBeenCalledTimes(1);
    expect(onActiveRunChange).toHaveBeenCalledWith("run-1");
    expect(onLoadMoreRuns).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <TeamRunPanel
          selectedTeam={buildTeam()}
          busy={null}
          onDeleteTeam={() => {}}
          runStatusFilter="completed"
          runStatusFilterOptions={[{ value: "completed", label: "Completed" }]}
          onRunStatusFilterChange={() => {}}
          onRefreshRuns={() => {}}
          runsLoading={false}
          visibleRuns={[]}
          activeRunId={null}
          onActiveRunChange={() => {}}
          isActiveRunHiddenByFilter={true}
          activeRun={buildRun({ id: "run-hidden" })}
          totalLoadedRunsForTeam={0}
          pageLimit={20}
          runsHasMore={false}
          selectedTeamId={null}
          onLoadMoreRuns={() => {}}
        />
      );
    });

    expect(container.textContent).toContain("Active run `run-hidden` is hidden by filter `completed`.");
    expect(container.textContent).toContain(
      "No runs loaded yet. Use Debug → Run Ops to create or load runs."
    );
  });

  it("TeamRunPanel no longer exposes create-run controls in primary surface", () => {
    act(() => {
      root.render(
        <TeamRunPanel
          selectedTeam={buildTeam()}
          busy={null}
          onDeleteTeam={() => {}}
          runStatusFilter="all"
          runStatusFilterOptions={[{ value: "all", label: "All" }]}
          onRunStatusFilterChange={() => {}}
          onRefreshRuns={() => {}}
          runsLoading={false}
          visibleRuns={[]}
          activeRunId={null}
          onActiveRunChange={() => {}}
          isActiveRunHiddenByFilter={false}
          activeRun={null}
          totalLoadedRunsForTeam={0}
          pageLimit={20}
          runsHasMore={false}
          selectedTeamId="team-1"
          onLoadMoreRuns={() => {}}
        />
      );
    });

    expect(container.textContent).toContain("Debug → Run Ops");
    expect(container.textContent).not.toContain("Create Run");
    expect(container.querySelector('textarea[aria-label="Run input JSON"]')).toBeNull();
  });

  it("TeamStepsPanel covers submit and all step action payload editors", () => {
    const onRefreshSteps = vi.fn();
    const onStepKeyChange = vi.fn();
    const onStepMemberIdChange = vi.fn();
    const onStepDependsOnChange = vi.fn();
    const onStepInputChange = vi.fn();
    const onSubmitStep = vi.fn();
    const onSelectedStepIdChange = vi.fn();
    const onStepActionChange = vi.fn();
    const onStepRemoteTaskIdChange = vi.fn();
    const onStepOutputChange = vi.fn();
    const onStepFailTextChange = vi.fn();
    const onStepInputReasonChange = vi.fn();
    const onStepInputRequiredPayloadChange = vi.fn();
    const onStepResumePayloadChange = vi.fn();
    const onApplyStepAction = vi.fn();

    const baseProps = {
      steps: [buildStep({ id: "step-1", status: "submitted" })],
      onRefreshSteps,
      stepKey: "plan",
      onStepKeyChange,
      stepMemberId: "worker-agent",
      onStepMemberIdChange,
      stepDependsOn: "",
      onStepDependsOnChange,
      stepInput: "{}",
      onStepInputChange,
      onSubmitStep,
      busy: null as string | null,
      selectedStepId: "step-1",
      onSelectedStepIdChange,
      stepAction: "start" as const,
      onStepActionChange,
      stepRemoteTaskId: "",
      onStepRemoteTaskIdChange,
      stepOutput: "{}",
      onStepOutputChange,
      stepFailText: "",
      onStepFailTextChange,
      stepInputReason: "",
      onStepInputReasonChange,
      stepInputRequiredPayload: "{}",
      onStepInputRequiredPayloadChange,
      stepResumePayload: "{}",
      onStepResumePayloadChange,
      onApplyStepAction,
    };

    act(() => {
      root.render(<TeamStepsPanel {...baseProps} />);
    });

    clickElement(findButtonByAriaLabel(container, "Refresh steps"));
    clickElement(findButtonByText(container, "Submit Step"));
    clickElement(findButtonByText(container, "Apply Step Action"));

    changeInputValue(
      required(container.querySelector('input[placeholder="step_key"]') as HTMLInputElement | null, "step_key input missing"),
      "dispatch"
    );
    changeInputValue(
      required(container.querySelector('input[placeholder="member_id"]') as HTMLInputElement | null, "member_id input missing"),
      "worker-2"
    );
    changeInputValue(
      required(
        container.querySelector('input[placeholder="depends_on (comma separated)"]') as HTMLInputElement | null,
        "depends_on input missing"
      ),
      "seed,plan"
    );
    changeInputValue(
      required(container.querySelector(".teams-step-panel textarea") as HTMLTextAreaElement | null, "step input textarea missing"),
      "{\"task\":\"ship\"}"
    );
    changeSelectValue(
      required(container.querySelectorAll(".teams-step-panel select")[0] as HTMLSelectElement | null, "step select missing"),
      "step-1"
    );
    changeSelectValue(
      required(container.querySelectorAll(".teams-step-panel select")[1] as HTMLSelectElement | null, "action select missing"),
      "fail"
    );
    changeInputValue(
      required(
        container.querySelector('input[placeholder="remote_task_id (optional)"]') as HTMLInputElement | null,
        "remote_task_id input missing"
      ),
      "task-9"
    );

    act(() => {
      root.render(<TeamStepsPanel {...baseProps} stepAction="complete" />);
    });
    changeInputValue(
      required(
        container.querySelectorAll(".teams-step-panel textarea")[1] as HTMLTextAreaElement | null,
        "complete output textarea missing"
      ),
      "{\"ok\":true}"
    );

    act(() => {
      root.render(<TeamStepsPanel {...baseProps} stepAction="fail" />);
    });
    changeInputValue(
      required(container.querySelector('input[placeholder="error_text"]') as HTMLInputElement | null, "error_text input missing"),
      "failed"
    );

    act(() => {
      root.render(<TeamStepsPanel {...baseProps} stepAction="input_required" />);
    });
    changeInputValue(
      required(container.querySelector('input[placeholder="reason (optional)"]') as HTMLInputElement | null, "reason input missing"),
      "need more context"
    );
    changeInputValue(
      required(
        container.querySelectorAll(".teams-step-panel textarea")[1] as HTMLTextAreaElement | null,
        "input_required payload textarea missing"
      ),
      "{\"question\":\"confirm\"}"
    );

    act(() => {
      root.render(<TeamStepsPanel {...baseProps} stepAction="resume" />);
    });
    changeInputValue(
      required(
        container.querySelectorAll(".teams-step-panel textarea")[1] as HTMLTextAreaElement | null,
        "resume payload textarea missing"
      ),
      "{\"answer\":\"yes\"}"
    );

    expect(onRefreshSteps).toHaveBeenCalledTimes(1);
    expect(onSubmitStep).toHaveBeenCalledTimes(1);
    expect(onApplyStepAction).toHaveBeenCalledTimes(1);
    expect(onStepKeyChange).toHaveBeenCalledWith("dispatch");
    expect(onStepMemberIdChange).toHaveBeenCalledWith("worker-2");
    expect(onStepDependsOnChange).toHaveBeenCalledWith("seed,plan");
    expect(onStepInputChange).toHaveBeenCalledWith('{"task":"ship"}');
    expect(onSelectedStepIdChange).toHaveBeenCalledWith("step-1");
    expect(onStepActionChange).toHaveBeenCalledWith("fail");
    expect(onStepRemoteTaskIdChange).toHaveBeenCalledWith("task-9");
    expect(onStepOutputChange).toHaveBeenCalledWith('{"ok":true}');
    expect(onStepFailTextChange).toHaveBeenCalledWith("failed");
    expect(onStepInputReasonChange).toHaveBeenCalledWith("need more context");
    expect(onStepInputRequiredPayloadChange).toHaveBeenCalledWith('{"question":"confirm"}');
    expect(onStepResumePayloadChange).toHaveBeenCalledWith('{"answer":"yes"}');
  });

  it("TeamStepsPanel supports list-only and controls-only modes", () => {
    act(() => {
      root.render(
        <TeamStepsPanel
          mode="list_only"
          steps={[buildStep({ id: "step-1", status: "working" })]}
          onRefreshSteps={() => {}}
          stepKey=""
          onStepKeyChange={() => {}}
          stepMemberId=""
          onStepMemberIdChange={() => {}}
          stepDependsOn=""
          onStepDependsOnChange={() => {}}
          stepInput="{}"
          onStepInputChange={() => {}}
          onSubmitStep={() => {}}
          busy={null}
          selectedStepId=""
          onSelectedStepIdChange={() => {}}
          stepAction="start"
          onStepActionChange={() => {}}
          stepRemoteTaskId=""
          onStepRemoteTaskIdChange={() => {}}
          stepOutput="{}"
          onStepOutputChange={() => {}}
          stepFailText=""
          onStepFailTextChange={() => {}}
          stepInputReason=""
          onStepInputReasonChange={() => {}}
          stepInputRequiredPayload="{}"
          onStepInputRequiredPayloadChange={() => {}}
          stepResumePayload="{}"
          onStepResumePayloadChange={() => {}}
          onApplyStepAction={() => {}}
        />
      );
    });
    expect(container.textContent).toContain("Step operations were moved to Debug -> Step Ops.");
    expect(container.querySelector(".teams-step-list")).not.toBeNull();
    expect(
      Array.from(container.querySelectorAll("button")).some((button) =>
        button.textContent?.includes("Submit Step")
      )
    ).toBe(false);

    act(() => {
      root.render(
        <TeamStepsPanel
          mode="controls_only"
          steps={[buildStep({ id: "step-1", status: "working" })]}
          onRefreshSteps={() => {}}
          stepKey=""
          onStepKeyChange={() => {}}
          stepMemberId=""
          onStepMemberIdChange={() => {}}
          stepDependsOn=""
          onStepDependsOnChange={() => {}}
          stepInput="{}"
          onStepInputChange={() => {}}
          onSubmitStep={() => {}}
          busy={null}
          selectedStepId=""
          onSelectedStepIdChange={() => {}}
          stepAction="start"
          onStepActionChange={() => {}}
          stepRemoteTaskId=""
          onStepRemoteTaskIdChange={() => {}}
          stepOutput="{}"
          onStepOutputChange={() => {}}
          stepFailText=""
          onStepFailTextChange={() => {}}
          stepInputReason=""
          onStepInputReasonChange={() => {}}
          stepInputRequiredPayload="{}"
          onStepInputRequiredPayloadChange={() => {}}
          stepResumePayload="{}"
          onStepResumePayloadChange={() => {}}
          onApplyStepAction={() => {}}
        />
      );
    });
    expect(findButtonByText(container, "Submit Step")).toBeDefined();
    expect(findButtonByText(container, "Apply Step Action")).toBeDefined();
    expect(container.querySelector(".teams-step-list")).toBeNull();
  });

  it("TeamEventsPanel supports auto-refresh toggle and load older actions", () => {
    const onEventsAutoRefreshChange = vi.fn();
    const onRefreshEvents = vi.fn();
    const onLoadOlderEvents = vi.fn();

    act(() => {
      root.render(
        <TeamEventsPanel
          eventsAutoRefresh={true}
          onEventsAutoRefreshChange={onEventsAutoRefreshChange}
          onRefreshEvents={onRefreshEvents}
          onLoadOlderEvents={onLoadOlderEvents}
          eventsLoading={false}
          previewMode={false}
          previewLimit={3}
          eventsHasMore={true}
          oldestEventId={1}
          displayedRunEvents={[buildRunEvent(7, { k: "v" })]}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    });

    toggleCheckboxValue(
      required(container.querySelector('input[type="checkbox"]') as HTMLInputElement | null, "events checkbox missing"),
      false
    );
    clickElement(findButtonByAriaLabel(container, "Refresh events"));
    clickElement(findButtonByText(container, "Load Older"));

    expect(onEventsAutoRefreshChange).toHaveBeenCalledWith(false);
    expect(onRefreshEvents).toHaveBeenCalledTimes(1);
    expect(onLoadOlderEvents).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <TeamEventsPanel
          eventsAutoRefresh={true}
          onEventsAutoRefreshChange={() => {}}
          onRefreshEvents={() => {}}
          onLoadOlderEvents={() => {}}
          eventsLoading={false}
          previewMode={true}
          previewLimit={5}
          eventsHasMore={false}
          oldestEventId={null}
          displayedRunEvents={[]}
          formatTs={(ts) => String(ts)}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    });

    expect(container.textContent).toContain("Showing latest 5 records.");
    expect(container.textContent).toContain("No events.");
  });

  it("TeamOverviewPanel refreshes snapshot and opens member mailbox", () => {
    const onRefreshSnapshot = vi.fn();
    const onOpenMailboxForMember = vi.fn();

    act(() => {
      root.render(
        <TeamOverviewPanel
          snapshot={buildSnapshot()}
          snapshotLoading={false}
          onRefreshSnapshot={onRefreshSnapshot}
          selectedMemberId="leader-agent"
          onOpenMailboxForMember={onOpenMailboxForMember}
        />
      );
    });

    clickElement(findButtonByAriaLabel(container, "Refresh snapshot"));
    clickElement(required(container.querySelectorAll(".teams-member-list .team-item")[1], "member button missing"));

    expect(onRefreshSnapshot).toHaveBeenCalledTimes(1);
    expect(onOpenMailboxForMember).toHaveBeenCalledWith("worker-agent");

    act(() => {
      root.render(
        <TeamOverviewPanel
          snapshot={null}
          snapshotLoading={false}
          onRefreshSnapshot={() => {}}
          selectedMemberId=""
          onOpenMailboxForMember={() => {}}
        />
      );
    });

    expect(container.textContent).toContain("No snapshot yet.");
  });

  it("TeamMemberConsolePanel switches preview and member-history views", () => {
    const onSelectedMemberIdChange = vi.fn();
    const onRefresh = vi.fn();
    const onLoadOlder = vi.fn();

    act(() => {
      root.render(
        <TeamMemberConsolePanel
          snapshot={buildSnapshot()}
          selectedMemberId=""
          onSelectedMemberIdChange={onSelectedMemberIdChange}
          selectedMemberSnapshot={null}
          memberEvents={[]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          displayedRunEvents={[buildRunEvent(9, { run: "preview" })]}
          previewLimit={5}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
        />
      );
    });

    changeSelectValue(
      required(container.querySelector("select") as HTMLSelectElement | null, "member select missing"),
      "worker-agent"
    );
    clickElement(findButtonByAriaLabel(container, "Refresh member console"));

    act(() => {
      root.render(
        <TeamMemberConsolePanel
          snapshot={buildSnapshot()}
          selectedMemberId="worker-agent"
          onSelectedMemberIdChange={onSelectedMemberIdChange}
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: null }),
            skills: ["team-worker-executor"],
          })}
          memberEvents={[]}
          memberEventsHasMore={true}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={1}
          displayedRunEvents={[]}
          previewLimit={5}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
        />
      );
    });

    clickElement(findButtonByText(container, "Load Older"));
    expect(container.textContent).toContain("Selected member has no associated session yet.");

    act(() => {
      root.render(
        <TeamMemberConsolePanel
          snapshot={buildSnapshot()}
          selectedMemberId="worker-agent"
          onSelectedMemberIdChange={onSelectedMemberIdChange}
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
          })}
          memberEvents={[]}
          memberEventsHasMore={true}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={1}
          displayedRunEvents={[]}
          previewLimit={5}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
        />
      );
    });

    expect(container.textContent).toContain("No member events yet.");

    const memberEvent: AgentEvent = {
      event_id: 11,
      agent_id: "agent-1",
      session_id: "session-1",
      seq: "11",
      ts: 100,
      stream: "stdout",
      message: "worker output",
    };

    act(() => {
      root.render(
        <TeamMemberConsolePanel
          snapshot={buildSnapshot()}
          selectedMemberId="worker-agent"
          onSelectedMemberIdChange={onSelectedMemberIdChange}
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
          })}
          memberEvents={[memberEvent]}
          memberEventsHasMore={true}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={1}
          displayedRunEvents={[]}
          previewLimit={5}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
        />
      );
    });

    expect(container.textContent).toContain("worker output");
    expect(onSelectedMemberIdChange).toHaveBeenCalledWith("worker-agent");
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onLoadOlder).toHaveBeenCalledTimes(1);
  });

  it("TeamMailboxPanel handles member chat, ack, and advanced mailbox controls", () => {
    const onSelectMember = vi.fn();
    const onConversationScroll = vi.fn();
    const onJumpToBottom = vi.fn();
    const onAckMessage = vi.fn();
    const onChatDraftChange = vi.fn();
    const onSendChatMessage = vi.fn();
    const onMsgFromActorIdChange = vi.fn();
    const onMsgToActorIdChange = vi.fn();
    const onMsgChannelChange = vi.fn();
    const onMsgTransportChange = vi.fn();
    const onMsgRouteChange = vi.fn();
    const onMsgTemplateChange = vi.fn();
    const onApplyMessageTemplate = vi.fn();
    const onMsgPayloadChange = vi.fn();
    const onMsgIdempotencyKeyChange = vi.fn();
    const onSendMessage = vi.fn();
    const onInboxActorIdChange = vi.fn();
    const onInboxLimitChange = vi.fn();
    const onInboxAfterIdChange = vi.fn();
    const onInboxIncludeDeliveredChange = vi.fn();
    const onRefreshInbox = vi.fn();
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    const pendingMessage = buildMailboxMessage(1);
    const deliveredMessage = buildMailboxMessage(2, {
      from_actor_id: "worker-agent",
      to_actor_id: "leader-agent",
      status: "delivered",
      payload: { type: "status_update", done: true },
      delivered_at: 1_700_000_200,
    });

    act(() => {
      root.render(
        <TeamMailboxPanel
          snapshot={buildSnapshot()}
          selectedMemberId="worker-agent"
          unreadByMemberId={{ "worker-agent": 2 }}
          onSelectMember={onSelectMember}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "worker-agent",
            inboxActorId: "worker-agent",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={onConversationScroll}
          onJumpToBottom={onJumpToBottom}
          conversationMessages={[pendingMessage, deliveredMessage]}
          toPrettyJson={toPrettyJson}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={onAckMessage}
          chatDraft="draft"
          onChatDraftChange={onChatDraftChange}
          onSendChatMessage={onSendChatMessage}
          msgFromActorId="leader-agent"
          onMsgFromActorIdChange={onMsgFromActorIdChange}
          msgToActorId="worker-agent"
          onMsgToActorIdChange={onMsgToActorIdChange}
          msgChannel="default"
          onMsgChannelChange={onMsgChannelChange}
          msgTransport="local"
          onMsgTransportChange={onMsgTransportChange}
          msgRoute="{}"
          onMsgRouteChange={onMsgRouteChange}
          mailboxTemplateOptions={[
            { value: "leader_task_assignment", label: "Leader Assignment" },
            { value: "worker_done", label: "Worker Done" },
          ]}
          msgTemplate="leader_task_assignment"
          onMsgTemplateChange={onMsgTemplateChange}
          onApplyMessageTemplate={onApplyMessageTemplate}
          msgPayload="{}"
          onMsgPayloadChange={onMsgPayloadChange}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={onMsgIdempotencyKeyChange}
          onSendMessage={onSendMessage}
          inboxActorId="worker-agent"
          onInboxActorIdChange={onInboxActorIdChange}
          inboxLimit="20"
          onInboxLimitChange={onInboxLimitChange}
          inboxAfterId=""
          onInboxAfterIdChange={onInboxAfterIdChange}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={onInboxIncludeDeliveredChange}
          onRefreshInbox={onRefreshInbox}
        />
      );
    });

    clickElement(required(container.querySelector(".teams-chat-members .team-item"), "member button missing"));

    act(() => {
      required(container.querySelector(".teams-chat-messages"), "chat list missing").dispatchEvent(
        new Event("scroll", { bubbles: true })
      );
    });

    clickElement(findButtonByText(container, "Ack"));
    clickElement(findButtonByText(container, "Jump to bottom"));

    const chatDraft = required(
      container.querySelector('textarea[placeholder="Type a message to selected agent"]') as
        | HTMLTextAreaElement
        | null,
      "chat draft textarea missing"
    );
    changeInputValue(chatDraft, "hello worker");
    act(() => {
      chatDraft.dispatchEvent(
        new KeyboardEvent("keydown", {
          bubbles: true,
          cancelable: true,
          key: "Enter",
          ctrlKey: true,
        })
      );
    });
    clickElement(findButtonByText(container, "Send Chat"));

    expect(onSelectMember).toHaveBeenCalledWith("leader-agent");
    expect(onConversationScroll).toHaveBeenCalledTimes(1);
    expect(onJumpToBottom).toHaveBeenCalledTimes(1);
    expect(onAckMessage).toHaveBeenCalledWith(pendingMessage);
    expect(onChatDraftChange).toHaveBeenCalledWith("hello worker");
    expect(onSendChatMessage).toHaveBeenCalledTimes(2);
    expect(toPrettyJson).toHaveBeenCalledWith({ type: "status_update", done: true });

    act(() => {
      root.render(
        <TeamMailboxPanel
          mode="advanced_only"
          snapshot={buildSnapshot()}
          selectedMemberId="worker-agent"
          unreadByMemberId={{ "worker-agent": 2 }}
          onSelectMember={onSelectMember}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "worker-agent",
            inboxActorId: "worker-agent",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={onConversationScroll}
          onJumpToBottom={onJumpToBottom}
          conversationMessages={[pendingMessage, deliveredMessage]}
          toPrettyJson={toPrettyJson}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={onAckMessage}
          chatDraft="draft"
          onChatDraftChange={onChatDraftChange}
          onSendChatMessage={onSendChatMessage}
          msgFromActorId="leader-agent"
          onMsgFromActorIdChange={onMsgFromActorIdChange}
          msgToActorId="worker-agent"
          onMsgToActorIdChange={onMsgToActorIdChange}
          msgChannel="default"
          onMsgChannelChange={onMsgChannelChange}
          msgTransport="local"
          onMsgTransportChange={onMsgTransportChange}
          msgRoute="{}"
          onMsgRouteChange={onMsgRouteChange}
          mailboxTemplateOptions={[
            { value: "leader_task_assignment", label: "Leader Assignment" },
            { value: "worker_done", label: "Worker Done" },
          ]}
          msgTemplate="leader_task_assignment"
          onMsgTemplateChange={onMsgTemplateChange}
          onApplyMessageTemplate={onApplyMessageTemplate}
          msgPayload="{}"
          onMsgPayloadChange={onMsgPayloadChange}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={onMsgIdempotencyKeyChange}
          onSendMessage={onSendMessage}
          inboxActorId="worker-agent"
          onInboxActorIdChange={onInboxActorIdChange}
          inboxLimit="20"
          onInboxLimitChange={onInboxLimitChange}
          inboxAfterId=""
          onInboxAfterIdChange={onInboxAfterIdChange}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={onInboxIncludeDeliveredChange}
          onRefreshInbox={onRefreshInbox}
        />
      );
    });

    changeInputValue(
      required(container.querySelector('input[placeholder="from_actor_id"]') as HTMLInputElement | null, "from_actor_id missing"),
      "leader-2"
    );
    changeInputValue(
      required(container.querySelector('input[placeholder="to_actor_id"]') as HTMLInputElement | null, "to_actor_id missing"),
      "worker-2"
    );
    changeInputValue(
      required(container.querySelector('input[placeholder="channel (default)"]') as HTMLInputElement | null, "channel missing"),
      "alerts"
    );
    changeSelectValue(
      required(container.querySelector('.teams-message-panel select') as HTMLSelectElement | null, "transport select missing"),
      "remote"
    );
    changeInputValue(
      required(
        container.querySelector('textarea[placeholder="route JSON (required for remote)"]') as
          | HTMLTextAreaElement
          | null,
        "route textarea missing"
      ),
      '{"hop":"edge"}'
    );
    changeSelectValue(
      required(
        container.querySelectorAll(".teams-message-panel select")[1] as HTMLSelectElement | null,
        "template select missing"
      ),
      "worker_done"
    );
    clickElement(findButtonByText(container, "Apply Template"));
    changeInputValue(
      required(container.querySelector('textarea[placeholder="payload JSON"]') as HTMLTextAreaElement | null, "payload textarea missing"),
      '{"type":"chat_message"}'
    );
    changeInputValue(
      required(
        container.querySelector('input[placeholder="idempotency_key (optional)"]') as HTMLInputElement | null,
        "idempotency input missing"
      ),
      "key-1"
    );
    clickElement(findButtonByText(container, "Send Message"));

    changeInputValue(
      required(container.querySelector('input[placeholder="actor_id"]') as HTMLInputElement | null, "inbox actor input missing"),
      "worker-2"
    );
    changeInputValue(
      required(container.querySelector('input[placeholder="limit"]') as HTMLInputElement | null, "inbox limit input missing"),
      "10"
    );
    changeInputValue(
      required(container.querySelector('input[placeholder="after_id (optional)"]') as HTMLInputElement | null, "after_id input missing"),
      "22"
    );
    toggleCheckboxValue(
      required(container.querySelector('.teams-message-panel input[type="checkbox"]') as HTMLInputElement | null, "include delivered checkbox missing"),
      true
    );
    clickElement(findButtonByAriaLabel(container, "Refresh inbox"));

    expect(onMsgFromActorIdChange).toHaveBeenCalledWith("leader-2");
    expect(onMsgToActorIdChange).toHaveBeenCalledWith("worker-2");
    expect(onMsgChannelChange).toHaveBeenCalledWith("alerts");
    expect(onMsgTransportChange).toHaveBeenCalledWith("remote");
    expect(onMsgRouteChange).toHaveBeenCalledWith('{"hop":"edge"}');
    expect(onMsgTemplateChange).toHaveBeenCalledWith("worker_done");
    expect(onApplyMessageTemplate).toHaveBeenCalledTimes(1);
    expect(onMsgPayloadChange).toHaveBeenCalledWith('{"type":"chat_message"}');
    expect(onMsgIdempotencyKeyChange).toHaveBeenCalledWith("key-1");
    expect(onSendMessage).toHaveBeenCalledTimes(1);
    expect(onInboxActorIdChange).toHaveBeenCalledWith("worker-2");
    expect(onInboxLimitChange).toHaveBeenCalledWith("10");
    expect(onInboxAfterIdChange).toHaveBeenCalledWith("22");
    expect(onInboxIncludeDeliveredChange).toHaveBeenCalledWith(true);
    expect(onRefreshInbox).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <TeamMailboxPanel
          snapshot={null}
          selectedMemberId=""
          unreadByMemberId={{}}
          onSelectMember={() => {}}
          chatActors={{ fromActorId: "", toActorId: "", inboxActorId: "" }}
          chatStickToBottom={false}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={() => {}}
          onJumpToBottom={() => {}}
          conversationMessages={[]}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => String(ts)}
          busy={null}
          onAckMessage={() => {}}
          chatDraft=""
          onChatDraftChange={() => {}}
          onSendChatMessage={() => {}}
          msgFromActorId=""
          onMsgFromActorIdChange={() => {}}
          msgToActorId=""
          onMsgToActorIdChange={() => {}}
          msgChannel=""
          onMsgChannelChange={() => {}}
          msgTransport="local"
          onMsgTransportChange={() => {}}
          msgRoute=""
          onMsgRouteChange={() => {}}
          mailboxTemplateOptions={[{ value: "leader_task_assignment", label: "Leader Assignment" }]}
          msgTemplate="leader_task_assignment"
          onMsgTemplateChange={() => {}}
          onApplyMessageTemplate={() => {}}
          msgPayload="{}"
          onMsgPayloadChange={() => {}}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={() => {}}
          onSendMessage={() => {}}
          inboxActorId=""
          onInboxActorIdChange={() => {}}
          inboxLimit="20"
          onInboxLimitChange={() => {}}
          inboxAfterId=""
          onInboxAfterIdChange={() => {}}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={() => {}}
          onRefreshInbox={() => {}}
        />
      );
    });

    expect(container.textContent).toContain("No members available.");
    expect(container.textContent).toContain("No conversation records yet for this pair.");
  });
});
