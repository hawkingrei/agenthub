// @vitest-environment jsdom
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MantineProvider } from "@mantine/core";
import {
  AgentEvent,
  TeamConversationMessageRecord,
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
import { TeamMemberAcpPanel } from "./team_member_acp_panel";
import { TeamActiveRunPanel } from "./team_active_run_panel";
import { TeamMemberConsolePanel } from "./team_member_console_panel";
import { TeamOverviewPanel } from "./team_overview_panel";
import { TeamTaskPanel } from "./team_task_panel";
import { TeamTasksPanel } from "./team_tasks_panel";
import { TeamRunPanel } from "./team_run_panel";
import { TeamSidebar } from "./team_sidebar";
import { TeamStepsPanel } from "./team_steps_panel";
import { TeamTabsBar } from "./team_tabs_bar";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

if (typeof window !== "undefined" && typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }) as MediaQueryList) as typeof window.matchMedia;
}

if (typeof globalThis.ResizeObserver !== "function") {
  globalThis.ResizeObserver = class ResizeObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  } as typeof ResizeObserver;
}

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

function queryButtonByText(container: HTMLElement, text: string): HTMLButtonElement | null {
  return (
    (Array.from(container.querySelectorAll("button")).find((candidate) =>
      candidate.textContent?.includes(text)
    ) as HTMLButtonElement | undefined) ?? null
  );
}

function findInteractiveByText(
  container: ParentNode,
  text: string,
  selectors = "button, label, [role='menuitem']"
): HTMLElement {
  const target = Array.from(container.querySelectorAll(selectors)).find((candidate) =>
    candidate.textContent?.includes(text)
  ) as HTMLElement | undefined;
  return required(target, `interactive element not found: ${text}`);
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
    runtime_handle_id: null,
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

function buildTaskMessage(
  messageId: number,
  overrides: Partial<TeamConversationMessageRecord> = {}
): TeamConversationMessageRecord {
  return {
    message_id: messageId,
    conversation_id: "conv-1",
    task_id: "task-1",
    from_actor_id: "user:u-1",
    to_actor_id: "leader-agent",
    route: "to_leader",
    payload: { type: "chat_message", text: `plan-${messageId}` },
    created_at: 1_700_000_100 + messageId,
    ...overrides,
  };
}

function buildPanelTask(
  id: string,
  overrides: Partial<{
    title: string;
    status: "open" | "in_progress" | "completed" | "canceled";
    context: Record<string, unknown>;
    created_at: number;
    updated_at: number;
  }> = {}
) {
  return {
    id,
    team_id: "team-1",
    title: overrides.title ?? id,
    status: overrides.status ?? "open",
    created_by_actor_id: "user",
    context: overrides.context ?? {},
    created_at: overrides.created_at ?? 1_700_000_000,
    updated_at: overrides.updated_at ?? 1_700_000_100,
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

function buildMemberLiveState(
  overrides: Partial<{
    member_id: string;
    role: string;
    agent_name?: string;
    lifecycle_status: string;
    lifecycle_tone: "active" | "inactive" | "missing";
    run_status: string;
    step_status: string;
    pending_inbox_count: number | null;
    current_work: string;
  }> = {}
) {
  return {
    member_id: "leader-agent",
    role: "leader",
    agent_name: "Leader Agent",
    lifecycle_status: "running",
    lifecycle_tone: "active" as const,
    run_status: "working",
    step_status: "working",
    pending_inbox_count: 1,
    current_work: "planning handoff",
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

  it("TeamSidebar renders subject rail and triggers navigation callbacks", () => {
    const onRefreshTeams = vi.fn();
    const onOpenCreateTeam = vi.fn();
    const onSelectTeam = vi.fn();
    const onSelectConversation = vi.fn();
    const onSelectKanban = vi.fn();
    const onSelectAgentTab = vi.fn();
    const onSelectUtilityTab = vi.fn();
    const teamOne = buildTeam();
    const teamTwo = buildTeam({ id: "team-2", name: "Team Two" });

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            developerMode={true}
            busy={null}
            onRefreshTeams={onRefreshTeams}
            onOpenCreateTeam={onOpenCreateTeam}
            draftTeamName="alpha"
            leaderMemberId="leader-agent"
            configuredWorkerCount={2}
            teams={[teamOne, teamTwo]}
            selectedTeam={teamOne}
            selectedTeamId="team-1"
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
              [
                "team-2",
                {
                  active: 2,
                  inactive: 0,
                  missing: 0,
                  total: 2,
                },
              ],
            ])}
            memberLiveStates={[
              buildMemberLiveState(),
              buildMemberLiveState({
                member_id: "worker-agent",
                role: "worker",
                agent_name: "Worker Agent",
                pending_inbox_count: 3,
                current_work: "collecting evidence",
              }),
            ]}
            focusedAgentMemberId="worker-agent"
            tab="member_console"
            onSelectTeam={onSelectTeam}
            onSelectConversation={onSelectConversation}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
            onSelectUtilityTab={onSelectUtilityTab}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Refresh teams"));
    clickElement(findButtonByAriaLabel(container, "Open team actions"));
    clickElement(findInteractiveByText(document.body, "Create Team"));
    const filterInput = required(
      container.querySelector("input[aria-label='Filter teams']"),
      "team filter input missing"
    ) as HTMLInputElement;
    expect(container.textContent).not.toContain("draft_team=alpha");
    changeInputValue(filterInput, "team-2");
    expect(container.textContent).toContain("filtered=1 total=2");
    clickElement(findButtonByAriaLabel(container, "Clear team filter"));
    clickElement(findButtonByAriaLabel(container, "Open team actions"));
    clickElement(findInteractiveByText(document.body, "Show Team Details"));
    expect(container.textContent).toContain("draft_team=alpha");
    expect(container.textContent).toContain("leader=leader-agent");
    expect(container.textContent).toContain("workers=2");
    clickElement(findButtonByText(container, "Team Two"));
    expect(container.querySelector("input[aria-label='Filter teams']")).not.toBeNull();
    expect(container.textContent).toContain("Team One");
    clickElement(findButtonByText(container, "Kanban"));
    clickElement(findButtonByAriaLabel(container, "Toggle agents section"));
    expect(container.textContent).not.toContain("Worker Agent");
    clickElement(findButtonByAriaLabel(container, "Toggle agents section"));
    expect(container.textContent).toContain("Worker Agent");
    clickElement(findButtonByText(container, "Shared team thread"));
    clickElement(findButtonByText(container, "Worker Agent"));

    expect(onRefreshTeams).toHaveBeenCalledTimes(1);
    expect(onOpenCreateTeam).toHaveBeenCalledTimes(1);
    expect(onSelectTeam).toHaveBeenCalledWith("team-2");
    expect(onSelectConversation).toHaveBeenCalledTimes(1);
    expect(onSelectKanban).toHaveBeenCalledTimes(1);
    expect(onSelectAgentTab).toHaveBeenCalledWith("worker-agent", "agent_acp");
    expect(container.textContent).toContain("Teams 2");
    expect(container.textContent).toContain("Kanban");
    expect(container.textContent).toContain("Agents");
    expect(container.textContent).toContain("Runs");
    expect(container.textContent).toContain("Advanced");
    expect(container.textContent).toContain("Leader Agent");
    expect(container.textContent).toContain("Worker Agent");
    expect(container.textContent).toContain("leader · working");
    expect(container.textContent).toContain("worker · working");
    expect(container.textContent).not.toContain("Console");

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            developerMode={false}
            busy={null}
            onRefreshTeams={onRefreshTeams}
            onOpenCreateTeam={onOpenCreateTeam}
            draftTeamName="alpha"
            leaderMemberId="leader-agent"
            configuredWorkerCount={2}
            teams={[teamOne, teamTwo]}
            selectedTeam={teamOne}
            selectedTeamId="team-1"
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
              [
                "team-2",
                {
                  active: 2,
                  inactive: 0,
                  missing: 0,
                  total: 2,
                },
              ],
            ])}
            memberLiveStates={[
              buildMemberLiveState(),
              buildMemberLiveState({
                member_id: "worker-agent",
                role: "worker",
                agent_name: "Worker Agent",
                pending_inbox_count: 3,
                current_work: "collecting evidence",
              }),
            ]}
            focusedAgentMemberId="worker-agent"
            tab="member_console"
            onSelectTeam={onSelectTeam}
            onSelectConversation={onSelectConversation}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
            onSelectUtilityTab={onSelectUtilityTab}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Open team actions"));
    expect(container.textContent).not.toContain("Show Team Details");
    expect(container.textContent).not.toContain("leader · working");
    expect(container.textContent).not.toContain("worker · working");
    expect(container.textContent).not.toContain("team-1");

    clickElement(findButtonByText(container, "Runs"));
    expect(onSelectUtilityTab).toHaveBeenCalledWith("runs");
    clickElement(findButtonByText(container, "Advanced"));
    expect(onSelectUtilityTab).toHaveBeenCalledWith("overview");
    expect(container.textContent).toContain("Teams 2");
    expect(container.textContent).toContain("Advanced");
    expect(container.textContent).toContain("Shared team thread");

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            developerMode={true}
            busy={null}
            onRefreshTeams={onRefreshTeams}
            onOpenCreateTeam={onOpenCreateTeam}
            draftTeamName="alpha"
            leaderMemberId="leader-agent"
            configuredWorkerCount={2}
            teams={[teamOne, teamTwo]}
            selectedTeam={teamOne}
            selectedTeamId={teamOne.id}
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
              [
                "team-2",
                {
                  active: 2,
                  inactive: 0,
                  missing: 0,
                  total: 2,
                },
              ],
            ])}
            memberLiveStates={[
              buildMemberLiveState(),
              buildMemberLiveState({
                member_id: "worker-agent",
                role: "worker",
                agent_name: "Worker Agent",
                pending_inbox_count: 3,
                current_work: "collecting evidence",
              }),
            ]}
            focusedAgentMemberId=""
            tab="runs"
            onSelectTeam={onSelectTeam}
            onSelectConversation={onSelectConversation}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
            onSelectUtilityTab={onSelectUtilityTab}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByText(container, "Shared team thread"));
    expect(onSelectConversation).toHaveBeenCalledTimes(2);

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            developerMode={false}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={() => {}}
            draftTeamName=""
            leaderMemberId=""
            configuredWorkerCount={0}
            teams={[buildTeam()]}
            selectedTeam={null}
            selectedTeamId={null}
            teamMemberSummaryByTeamId={new Map()}
            memberLiveStates={[]}
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={() => {}}
            onSelectConversation={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
            onSelectUtilityTab={() => {}}
          />
        </MantineProvider>
      );
    });

    const unmatchedFilterInput = required(
      container.querySelector("input[aria-label='Filter teams']"),
      "team filter input missing"
    ) as HTMLInputElement;
    changeInputValue(unmatchedFilterInput, "missing-team");
    expect(container.textContent).toContain("No teams match current filter.");

    const noTeamsCreate = vi.fn();
    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            developerMode={false}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={noTeamsCreate}
            draftTeamName=""
            leaderMemberId=""
            configuredWorkerCount={0}
            teams={[]}
            selectedTeam={null}
            selectedTeamId={null}
            teamMemberSummaryByTeamId={new Map()}
            memberLiveStates={[]}
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={() => {}}
            onSelectConversation={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
            onSelectUtilityTab={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("No teams yet.");
    expect(container.querySelector("input[aria-label='Filter teams']")).toBeNull();
    clickElement(findButtonByText(container, "Create Team"));
    expect(noTeamsCreate).toHaveBeenCalledTimes(1);
  });

  it("TeamRunPanel supports run filter/list interactions and empty-state messages", () => {
    const onDeleteTeam = vi.fn();
    const onStartRun = vi.fn();
    const onRunStatusFilterChange = vi.fn();
    const onRefreshRuns = vi.fn();
    const onActiveRunChange = vi.fn();
    const onLoadMoreRuns = vi.fn();

    const activeRun = buildRun({ id: "run-1" });

    act(() => {
      root.render(
        <MantineProvider>
          <TeamRunPanel
            selectedTeam={buildTeam()}
            developerMode={true}
            busy={null}
            onDeleteTeam={onDeleteTeam}
            onStartRun={onStartRun}
            canStartRun={true}
            runBlockedReason={null}
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
        </MantineProvider>
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
    clickElement(findButtonByText(container, "Start Run"));
    clickElement(findButtonByAriaLabel(container, "Refresh runs"));
    clickElement(required(container.querySelector(".teams-run-list .team-item"), "run list item missing"));
    clickElement(findButtonByText(container, "Load More"));

    expect(onDeleteTeam).toHaveBeenCalledTimes(1);
    expect(onStartRun).toHaveBeenCalledTimes(1);
    expect(onRunStatusFilterChange).toHaveBeenCalledWith("working");
    expect(onRefreshRuns).toHaveBeenCalledTimes(1);
    expect(onActiveRunChange).toHaveBeenCalledWith("run-1");
    expect(onLoadMoreRuns).toHaveBeenCalledTimes(1);

    act(() => {
      root.render(
        <MantineProvider>
          <TeamRunPanel
            selectedTeam={buildTeam()}
            developerMode={true}
            busy={null}
            onDeleteTeam={() => {}}
            onStartRun={() => {}}
            canStartRun={false}
            runBlockedReason="Add at least one agent before starting the team runtime or a run."
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
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Active run `run-hidden` is hidden by filter `completed`.");
    expect(container.textContent).toContain(
      "No runs loaded yet. Use Debug → Run Ops to create or load runs."
    );
  });

  it("TeamSidebar hides selector controls in detail mode", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            showTeamSelector={false}
            developerMode={false}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={() => {}}
            draftTeamName=""
            leaderMemberId="leader-agent"
            configuredWorkerCount={1}
            teams={[buildTeam()]}
            selectedTeam={buildTeam({ name: "Detail Team" })}
            selectedTeamId="team-1"
            teamMemberSummaryByTeamId={new Map([
              [
                "team-1",
                {
                  active: 1,
                  inactive: 0,
                  missing: 0,
                  total: 1,
                },
              ],
            ])}
            memberLiveStates={[buildMemberLiveState()]}
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={() => {}}
            onSelectConversation={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
            onSelectUtilityTab={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Detail Team");
    expect(container.textContent).not.toContain("Browse this team's channels, members, and operations.");
    expect(container.textContent).not.toContain("Team Selector");
    expect(container.querySelector("input[aria-label='Filter teams']")).toBeNull();
    expect(container.textContent).not.toContain("Teams 1");
    expect(container.textContent).not.toContain("Create Team");
    expect(container.textContent).not.toContain("Shared team thread");
    expect(container.textContent).not.toContain("Task board");
  });

  it("TeamRunPanel no longer exposes create-run controls in primary surface", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamRunPanel
            selectedTeam={buildTeam()}
            developerMode={false}
            busy={null}
            onDeleteTeam={() => {}}
            onStartRun={() => {}}
            canStartRun={false}
            runBlockedReason="Add at least one agent before starting the team runtime or a run."
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
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain(
      "Add at least one agent before starting the team runtime or a run."
    );
    expect(container.textContent).not.toContain("Debug → Run Ops");
    expect(container.textContent).toContain("Start Run");
    expect(container.textContent).not.toContain("Create Run");
    expect(container.querySelector('textarea[aria-label="Run input JSON"]')).toBeNull();
  });

  it("TeamTabsBar renders product tabs and switches selected tab", () => {
    const onTabChange = vi.fn();
    act(() => {
      root.render(<TeamTabsBar tab="runs" onTabChange={onTabChange} />);
    });

    expect(container.textContent).toContain("Runs");
    expect(container.textContent).toContain("Conversation");
    expect(container.textContent).toContain("Agent ACP");
    expect(container.textContent).toContain("Debug");

    clickElement(findButtonByText(container, "Events"));
    expect(onTabChange).toHaveBeenCalledWith("events");
  });

  it("TeamActiveRunPanel exposes run actions and metadata", () => {
    const onRefresh = vi.fn();
    const onCancel = vi.fn();
    const onResume = vi.fn();
    const onRestart = vi.fn();
    act(() => {
      root.render(
        <TeamActiveRunPanel
          run={buildRun({ status: "failed" })}
          busy={null}
          canResumeRun={true}
          canRestartRun={true}
          onRefresh={onRefresh}
          onCancel={onCancel}
          onResume={onResume}
          onRestart={onRestart}
          formatTs={(value) => (value == null ? "-" : String(value))}
          cardClassName="card"
          titleClassName="title"
          metaItemClassName="meta"
        />
      );
    });

    expect(container.textContent).toContain("Active Run");
    expect(container.textContent).toContain("run-1");
    expect(container.textContent).toContain("ctx-1");
    clickElement(findButtonByAriaLabel(container, "Refresh active run"));
    clickElement(findButtonByText(container, "Cancel Run"));
    clickElement(findButtonByText(container, "Resume Run"));
    clickElement(findButtonByText(container, "Restart Run"));
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onResume).toHaveBeenCalledTimes(1);
    expect(onRestart).toHaveBeenCalledTimes(1);
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
      developerMode: true,
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
        container.querySelector('input[placeholder="runtime_handle_id (optional)"]') as HTMLInputElement | null,
        "runtime_handle_id input missing"
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
          developerMode={true}
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
          developerMode={true}
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

    act(() => {
      root.render(
        <TeamStepsPanel
          developerMode={false}
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
    expect(container.textContent).toContain("Step controls are available in Developer Mode.");
    expect(container.querySelector(".teams-step-list")).not.toBeNull();
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
    expect(container.textContent).toContain("Cold Start Playbook");
    expect(container.textContent).toContain("Leader startup");
    expect(container.textContent).toContain("Worker startup");

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
          memberDiscoveryCard={{
            card_id: "agenthub://agents/worker-agent",
            schema_version: "agenthub.a2a.discovery_card.v1",
            description:
              "AgentHub team member worker-agent (provider: codex) supports team_mailbox_v1, acp_codex",
            identity: {
              agent_id: "worker-agent",
              name: "worker-agent",
              status: "running",
            },
            runtime: {
              acp_provider: "codex",
              code_mode: true,
              worktree_mode: "create_worktree",
              worktree_repo: "/tmp/repo",
              worktree_ref: "main",
            },
            capability_tags: ["team_mailbox_v1", "acp_codex"],
          }}
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
          memberDiscoveryCard={{
            card_id: "agenthub://agents/worker-agent",
            schema_version: "agenthub.a2a.discovery_card.v1",
            description:
              "AgentHub team member worker-agent (provider: codex) supports team_mailbox_v1, acp_codex",
            identity: {
              agent_id: "worker-agent",
              name: "worker-agent",
              status: "running",
            },
            runtime: {
              acp_provider: "codex",
              code_mode: true,
              worktree_mode: "create_worktree",
              worktree_repo: "/tmp/repo",
              worktree_ref: "main",
            },
            capability_tags: ["team_mailbox_v1", "acp_codex"],
          }}
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
          memberDiscoveryCard={{
            card_id: "agenthub://agents/worker-agent",
            schema_version: "agenthub.a2a.discovery_card.v1",
            description:
              "AgentHub team member worker-agent (provider: codex) supports team_mailbox_v1, acp_codex",
            identity: {
              agent_id: "worker-agent",
              name: "worker-agent",
              status: "running",
            },
            runtime: {
              acp_provider: "codex",
              code_mode: true,
              worktree_mode: "create_worktree",
              worktree_repo: "/tmp/repo",
              worktree_ref: "main",
            },
            capability_tags: ["team_mailbox_v1", "acp_codex"],
          }}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
        />
      );
    });

    expect(container.textContent).toContain("worker output");
    expect(container.textContent).toContain("acp_codex");
    expect(container.textContent).toContain(
      "AgentHub team member worker-agent (provider: codex) supports team_mailbox_v1, acp_codex"
    );
    expect(onSelectedMemberIdChange).toHaveBeenCalledWith("worker-agent");
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onLoadOlder).toHaveBeenCalled();
  });

  it("TeamTaskPanel supports create/select/send workflow", () => {
    const onRefreshTasks = vi.fn();
    const onMessageDraftChange = vi.fn();
    const onSendMessage = vi.fn();
    const onRefreshMessages = vi.fn();
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    function TeamTaskPanelHarness() {
      const [draft, setDraft] = React.useState("please continue @Worker Agent");
      return (
        <TeamTaskPanel
          developerMode={true}
          tasksLoading={false}
          onRefreshTasks={onRefreshTasks}
          messageDraft={draft}
          onMessageDraftChange={(value) => {
            onMessageDraftChange(value);
            setDraft(value);
          }}
          onSendMessage={onSendMessage}
          onRefreshMessages={onRefreshMessages}
          messages={[
            buildTaskMessage(1),
            buildTaskMessage(2, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "status_update", done: true },
            }),
          ]}
          memberLiveStates={[
            {
              member_id: "leader-agent",
              role: "leader",
              agent_name: "Leader Agent",
              lifecycle_status: "running",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "planning",
            },
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "Worker Agent",
              lifecycle_status: "running",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "blocked",
              pending_inbox_count: 1,
              current_work: "blocked on dependency",
            },
          ]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );
    }

    act(() => {
      root.render(<TeamTaskPanelHarness />);
    });

    clickElement(findButtonByAriaLabel(container, "Toggle thread options"));
    clickElement(findButtonByText(container, "Refresh Channel"));
    clickElement(findButtonByText(container, "Refresh Thread"));
    changeInputValue(
      required(
        container.querySelector(
          'textarea[placeholder="Message #all"]'
        ) as HTMLTextAreaElement | null,
        "draft textarea missing"
      ),
      "please continue @Worker Agent and review"
    );
    clickElement(findButtonByText(container, "Send"));

    expect(onRefreshTasks).toHaveBeenCalledTimes(1);
    expect(onRefreshMessages).toHaveBeenCalledTimes(1);
    expect(onSendMessage).toHaveBeenCalledTimes(1);
    expect(onSendMessage).toHaveBeenCalledWith({
      text: "please continue <at>worker-agent</at> and review",
      mentionActorIds: ["worker-agent"],
    });
    expect(onMessageDraftChange).toHaveBeenCalledWith("please continue @Worker Agent and review");
    expect(toPrettyJson).toHaveBeenCalledWith({ type: "status_update", done: true });
    expect(container.textContent).not.toContain("(task-1)");
    expect(container.textContent).not.toContain("conversation_id=task-1");
    expect(container.querySelector("h3")).toBeNull();
    expect(container.textContent).not.toContain(
      "General channel for shared planning, requests, and broadcast coordination."
    );
    expect(container.textContent).toContain(
      "Use @name for direct replies · Ctrl/Cmd + Enter to send"
    );
    expect(container.textContent).not.toContain("worker update");
    expect(container.textContent).not.toContain("work:working");
    expect(container.textContent).not.toContain("agent:working");
    const detailButtons = Array.from(container.querySelectorAll("button")).filter((candidate) =>
      candidate.textContent?.includes("Show details")
    );
    clickElement(detailButtons[1] ?? null);
    expect(container.textContent).toContain("work");
    expect(container.textContent).toContain("working/working");
    expect(container.textContent).toContain("agent");
    expect(container.textContent).toContain("running");
  });

  it("TeamTaskPanel renders canonical agent replies already persisted in shared thread", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    act(() => {
      root.render(
        <TeamTaskPanel
          developerMode={true}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          onRefreshMessages={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "user:u-1",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "hello team" },
            }),
            buildTaskMessage(2, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "leader reply visible in all" },
            }),
          ]}
          seenByMessageId={{ 1: ["leader-agent", "worker-agent"] }}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "leader-agent",
              role: "leader",
              agent_name: "LeaderAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "reviewing shared thread",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );
    });

    clickElement(findButtonByText(container, "Seen by 2 agents"));
    expect(container.textContent).toContain("hello team");
    expect(container.textContent).toContain("leader reply visible in all");
    expect(container.textContent).not.toContain("Delivery pending");
    expect(container.textContent).toContain("You");
    expect(container.textContent).toContain("LeaderAgent");
    expect(container.textContent).toContain("worker-agent");
    const activityKinds = Array.from(
      container.querySelectorAll("[data-activity-author-kind]")
    ).map((node) => node.getAttribute("data-activity-author-kind"));
    expect(activityKinds).toEqual(["human", "agent"]);
  });

  it("TeamTaskPanel sticks to bottom by default and shows a jump action after manual upward scroll", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const rafSpy = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback: FrameRequestCallback) => {
        callback(0);
        return 1;
      });
    const cancelSpy = vi
      .spyOn(window, "cancelAnimationFrame")
      .mockImplementation(() => {});

    try {
      act(() => {
        root.render(
          <TeamTaskPanel
            developerMode={false}
            tasksLoading={false}
            onRefreshTasks={vi.fn()}
            messageDraft=""
            onMessageDraftChange={vi.fn()}
            onSendMessage={vi.fn()}
            onRefreshMessages={vi.fn()}
            messages={Array.from({ length: 12 }, (_, index) =>
              buildTaskMessage(index + 1, {
                from_actor_id: index === 0 ? "user:u-1" : "leader-agent",
                to_actor_id: null,
                route: "group_chat",
                payload: { type: "chat_message", text: `message ${index + 1}` },
              })
            )}
            humanActorId="user"
            memberLiveStates={[]}
            memberIds={["leader-agent"]}
            messagesLoading={false}
            busy={null}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={toPrettyJson}
          />
        );
      });

      const scrollNode = required(
        container.querySelector('[data-team-channel-scroll="true"]') as HTMLDivElement | null,
        "team channel scroll container missing"
      );
      Object.defineProperty(scrollNode, "scrollHeight", {
        configurable: true,
        value: 640,
      });
      Object.defineProperty(scrollNode, "clientHeight", {
        configurable: true,
        value: 200,
      });
      Object.defineProperty(scrollNode, "scrollTop", {
        configurable: true,
        writable: true,
        value: 0,
      });

      expect(findButtonByText(container, "Jump to top")).not.toBeNull();

      act(() => {
        root.render(
          <TeamTaskPanel
            developerMode={false}
            tasksLoading={false}
            onRefreshTasks={vi.fn()}
            messageDraft=""
            onMessageDraftChange={vi.fn()}
            onSendMessage={vi.fn()}
            onRefreshMessages={vi.fn()}
            messages={Array.from({ length: 13 }, (_, index) =>
              buildTaskMessage(index + 1, {
                from_actor_id:
                  index === 0 ? "user:u-1" : index === 12 ? "worker-agent" : "leader-agent",
                to_actor_id: null,
                route: "group_chat",
                payload: { type: "chat_message", text: `message ${index + 1}` },
              })
            )}
            humanActorId="user"
            memberLiveStates={[]}
            memberIds={["leader-agent", "worker-agent"]}
            messagesLoading={false}
            busy={null}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={toPrettyJson}
          />
        );
      });

      await act(async () => {
        await Promise.resolve();
      });

      expect(scrollNode.scrollTop).toBe(640);
      expect(queryButtonByText(container, "Jump to bottom")).toBeNull();
      expect(queryButtonByText(container, "Jump to top")).not.toBeNull();

      act(() => {
        clickElement(findButtonByText(container, "Jump to top"));
      });
      expect(scrollNode.scrollTop).toBe(0);
      act(() => {
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });
      expect(queryButtonByText(container, "Jump to bottom")).not.toBeNull();

      act(() => {
        scrollNode.scrollTop = 80;
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });

      const jumpButton = queryButtonByText(container, "Jump to bottom");
      expect(jumpButton).not.toBeNull();
      expect(queryButtonByText(container, "Jump to top")).toBeNull();

      clickElement(jumpButton);
      expect(scrollNode.scrollTop).toBe(640);
      expect(queryButtonByText(container, "Jump to bottom")).toBeNull();
      expect(queryButtonByText(container, "Jump to top")).not.toBeNull();
    } finally {
      rafSpy.mockRestore();
      cancelSpy.mockRestore();
    }
  });

  it("TeamTaskPanel renders canonical stringified chat payloads as thread text", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    act(() => {
      root.render(
        <TeamTaskPanel
          developerMode={true}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          onRefreshMessages={vi.fn()}
          messages={[
            buildTaskMessage(11, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload:
                '{"type":"chat_message","current_phase":"Team formation","text":"rendered from string payload"}',
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["leader-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );
    });

    expect(container.textContent).toContain("rendered from string payload");
    expect(container.textContent).not.toContain('{"type":"chat_message"');
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamTaskPanel hides message details when developer mode is off", () => {
    act(() => {
      root.render(
        <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          onRefreshMessages={vi.fn()}
          messages={[buildTaskMessage(1)]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    });

    expect(container.textContent).not.toContain("Show details");
    expect(container.textContent).not.toContain("source");
    expect(container.textContent).not.toContain("route");
  });

  it("TeamTasksPanel supports task filters, creation, linked runs, and debug compile actions", () => {
    const onSelectedTaskIdChange = vi.fn();
    const onRefreshTasks = vi.fn();
    const onNewTaskTitleChange = vi.fn();
    const onCreateTask = vi.fn();
    const onUpdateTaskStatus = vi.fn();
    const onCompilePreviewContextIdChange = vi.fn();
    const onCompileTaskRunPreview = vi.fn();
    const onUseCompiledRunPayload = vi.fn();
    const onCreateRunFromCompiledPreview = vi.fn();
    const onOpenRun = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            developerMode={true}
            tasks={[
              buildPanelTask("task-1", {
                title: "Investigate bug",
                status: "open",
                created_at: 100,
                updated_at: 200,
              }),
              buildPanelTask("task-2", {
                title: "Prepare rollout",
                status: "in_progress",
                context: { owner: "leader" },
                created_at: 90,
                updated_at: 220,
              }),
            ]}
            tasksLoading={false}
            selectedTaskId="task-2"
            onSelectedTaskIdChange={onSelectedTaskIdChange}
            onRefreshTasks={onRefreshTasks}
            newTaskTitle="New task draft"
            onNewTaskTitleChange={onNewTaskTitleChange}
            onCreateTask={onCreateTask}
            onUpdateTaskStatus={onUpdateTaskStatus}
            busy={null}
            runs={[
              buildRun({
                id: "run-2",
                status: "completed",
                summary: "Shipped the rollout summary.",
                input: { task_id: "task-2" },
                created_at: 230,
                started_at: 231,
                ended_at: 240,
              }),
            ]}
            onOpenRun={onOpenRun}
            compilePreviewContextId="ctx-preview"
            onCompilePreviewContextIdChange={onCompilePreviewContextIdChange}
            onCompileTaskRunPreview={onCompileTaskRunPreview}
            canCompileTask={true}
            compiledRunPreview={{
              conversation_id: "conv-77",
              run_payload: {
                context_id: "ctx-preview",
                input: { objective: "ship" },
              },
              plan: { steps: ["review", "ship"] },
            }}
            onUseCompiledRunPayload={onUseCompiledRunPayload}
            onCreateRunFromCompiledPreview={onCreateRunFromCompiledPreview}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={(value) => JSON.stringify(value)}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Refresh tasks"));
    clickElement(findButtonByText(container, "Investigate bug"));
    clickElement(findButtonByAriaLabel(container, "Start Investigate bug"));
    clickElement(findInteractiveByText(container, "In progress", "button, label"));
    changeInputValue(
      required(
        container.querySelector('input[placeholder="New task title"]') as HTMLInputElement | null,
        "new task input missing"
      ),
      "Create changelog"
    );
    clickElement(findButtonByText(container, "New Task"));
    clickElement(findInteractiveByText(container, "Developer tools", "summary"));
    changeInputValue(
      required(
        container.querySelector(
          'input[placeholder="context_id override (optional)"]'
        ) as HTMLInputElement | null,
        "context input missing"
      ),
      "ctx-next"
    );
    clickElement(findButtonByText(container, "Compile Preview"));
    clickElement(findButtonByText(container, "Use Payload in Create Run"));
    clickElement(findButtonByText(container, "Create Run from Preview"));
    clickElement(findButtonByText(container, "Open Run"));

    expect(onRefreshTasks).toHaveBeenCalledTimes(1);
    expect(onSelectedTaskIdChange).toHaveBeenCalledWith("task-1");
    expect(onNewTaskTitleChange).toHaveBeenCalledWith("Create changelog");
    expect(onCreateTask).toHaveBeenCalledTimes(1);
    expect(onUpdateTaskStatus).toHaveBeenCalledWith("task-1", "in_progress");
    expect(onCompilePreviewContextIdChange).toHaveBeenCalledWith("ctx-next");
    expect(onCompileTaskRunPreview).toHaveBeenCalledTimes(1);
    expect(onUseCompiledRunPayload).toHaveBeenCalledTimes(1);
    expect(onCreateRunFromCompiledPreview).toHaveBeenCalledTimes(1);
    expect(onOpenRun).toHaveBeenCalledWith("run-2");
    expect(container.textContent).toContain("Kanban");
    expect(container.textContent).toContain("Board lanes");
    expect(container.textContent).toContain("Completed");
    expect(container.textContent).toContain("Prepare rollout");
    expect(container.textContent).toContain("Latest run");
    expect(container.textContent).toContain("Shipped the rollout summary.");
    expect(container.textContent).toContain("Task context");
  });

  it("TeamTasksPanel keeps details aligned with the active filter", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            developerMode={false}
            tasks={[
              buildPanelTask("task-open", { title: "Investigate bug", status: "open" }),
              buildPanelTask("task-progress", {
                title: "Prepare rollout",
                status: "in_progress",
              }),
            ]}
            tasksLoading={false}
            selectedTaskId="task-progress"
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            newTaskTitle=""
            onNewTaskTitleChange={vi.fn()}
            onCreateTask={vi.fn()}
            onUpdateTaskStatus={vi.fn()}
            busy={null}
            runs={[]}
            onOpenRun={vi.fn()}
            compilePreviewContextId=""
            onCompilePreviewContextIdChange={vi.fn()}
            onCompileTaskRunPreview={vi.fn()}
            canCompileTask={false}
            compiledRunPreview={null}
            onUseCompiledRunPayload={vi.fn()}
            onCreateRunFromCompiledPreview={vi.fn()}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={(value) => JSON.stringify(value)}
          />
        </MantineProvider>
      );
    });

    clickElement(findInteractiveByText(container, "Open", "button, label"));
    expect(container.textContent).toContain("Investigate bug");
    expect(container.textContent).not.toContain("Prepare rolloutAgents pick this task up automatically");
  });

  it("TeamMemberAcpPanel renders ACP conversation for selected member", () => {
    const onRefresh = vi.fn();
    const onLoadOlder = vi.fn();
    const acpEvents: AgentEvent[] = [
      {
        event_id: 21,
        agent_id: "worker-agent",
        session_id: "task-77",
        seq: "21",
        ts: 1_700_000_201,
        stream: "acp",
        message: JSON.stringify({
          type: "user_message",
          text: "Please investigate this issue.",
        }),
      },
      {
        event_id: 22,
        agent_id: "worker-agent",
        session_id: "task-77",
        seq: "22",
        ts: 1_700_000_202,
        stream: "acp",
        message: JSON.stringify({
          type: "agent_message",
          text: "Acknowledged. I am checking logs now.",
        }),
      },
    ];

    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
          })}
          memberEvents={acpEvents}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={20}
          onRefresh={onRefresh}
          onLoadOlder={onLoadOlder}
        />
      );
    });

    expect(container.textContent).not.toContain("member=worker-agent");
    expect(container.textContent).not.toContain("role=worker");
    expect(container.textContent).not.toContain("session=task-77");
    clickElement(findButtonByAriaLabel(container, "Toggle thread options"));
    clickElement(findButtonByText(container, "Refresh Thread"));
    clickElement(findButtonByText(container, "Load Older"));
    expect(container.querySelector("h3")).toBeNull();
    expect(container.textContent).toContain("Conversation");
    expect(container.textContent).toContain("Plan");
    expect(container.textContent).toContain("Debug");
    expect(container.textContent).toContain("Please investigate this issue.");
    expect(container.textContent).toContain("Acknowledged. I am checking logs now.");
    expect(container.textContent).toContain("member=worker-agent");
    expect(container.textContent).toContain("role=worker");
    expect(container.textContent).toContain("session=task-77");
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(onLoadOlder).toHaveBeenCalledTimes(0);
  });

  it("TeamMemberAcpPanel auto-loads older ACP history for short threads and renders agent thinking", async () => {
    const onLoadOlder = vi.fn();

    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
          })}
          memberEvents={[
            {
              event_id: 23,
              agent_id: "worker-agent",
              session_id: "task-77",
              seq: "23",
              ts: 1_700_000_203,
              stream: "acp",
              message: JSON.stringify({
                type: "agent_thought",
                text: "Inspecting the previous failure before replying.",
              }),
            },
            {
              event_id: 24,
              agent_id: "worker-agent",
              session_id: "task-77",
              seq: "24",
              ts: 1_700_000_204,
              stream: "acp",
              message: JSON.stringify({
                type: "agent_message",
                text: "I found the relevant stack trace.",
              }),
            },
          ]}
          memberEventsHasMore={true}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={22}
          onRefresh={vi.fn()}
          onLoadOlder={onLoadOlder}
        />
      );
    });

    await act(async () => {
      await Promise.resolve();
    });

    expect(onLoadOlder).toHaveBeenCalled();
    expect(container.textContent).toContain(
      "Inspecting the previous failure before replying."
    );
    expect(container.textContent).toContain("I found the relevant stack trace.");
  });

  it("TeamMemberAcpPanel hides technical metadata when developer mode is off", () => {
    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={false}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={buildMemberSnapshot({
            member_id: "worker-agent",
            role: "worker",
            latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
          })}
          memberEvents={[
            {
              event_id: 25,
              agent_id: "worker-agent",
              session_id: "task-77",
              seq: "25",
              ts: 1_700_000_205,
              stream: "acp",
              message: JSON.stringify({
                type: "agent_message",
                text: "Panel chrome should still render without debug tab.",
              }),
            },
          ]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onRefresh={vi.fn()}
          onLoadOlder={vi.fn()}
        />
      );
    });

    clickElement(findButtonByAriaLabel(container, "Toggle thread options"));
    expect(container.textContent).toContain("Conversation");
    expect(container.textContent).toContain("Plan");
    expect(container.textContent).not.toContain("Debug");
    expect(container.textContent).toContain("Refresh Thread");
    expect(container.textContent).not.toContain("member=worker-agent");
    expect(container.textContent).not.toContain("role=worker");
    expect(container.textContent).not.toContain("session=task-77");
  });

  it("TeamMemberAcpPanel renders ACP conversation from runtime session fallback", () => {
    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={null}
          selectedMemberRole="worker"
          selectedSessionId="runtime-session-1"
          memberEvents={[
            {
              event_id: 31,
              agent_id: "worker-agent",
              session_id: "runtime-session-1",
              seq: "31",
              ts: 1_700_000_301,
              stream: "acp",
              message: JSON.stringify({
                type: "agent_message",
                text: "Runtime session fallback works.",
              }),
            },
          ]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onRefresh={vi.fn()}
          onLoadOlder={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain("Runtime session fallback works.");
  });

  it("TeamMemberAcpPanel sends prompt through ACP input dock", async () => {
    const onSendInput = vi.fn().mockResolvedValue(undefined);

    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={null}
          selectedMemberRole="worker"
          selectedSessionId="runtime-session-1"
          memberEvents={[]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onSendInput={onSendInput}
          onRefresh={vi.fn()}
          onLoadOlder={vi.fn()}
        />
      );
    });

    const input = required(
      container.querySelector("textarea") as HTMLTextAreaElement | null,
      "ACP input textarea missing"
    );
    changeInputValue(input, "hello from team acp");
    await act(async () => {
      findButtonByText(container, "Send").dispatchEvent(
        new MouseEvent("click", { bubbles: true, cancelable: true })
      );
      await Promise.resolve();
    });

    expect(onSendInput).toHaveBeenCalledWith("hello from team acp", "runtime-session-1");
  });

  it("TeamMemberAcpPanel ignores duplicate send triggers while a prompt is in flight", async () => {
    let resolveSend: (() => void) | null = null;
    const onSendInput = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSend = resolve;
        })
    );

    act(() => {
      root.render(
        <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          selectedMemberSnapshot={null}
          selectedMemberRole="worker"
          selectedSessionId="runtime-session-1"
          memberEvents={[]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onSendInput={onSendInput}
          onRefresh={vi.fn()}
          onLoadOlder={vi.fn()}
        />
      );
    });

    const input = required(
      container.querySelector("textarea") as HTMLTextAreaElement | null,
      "ACP input textarea missing"
    );
    changeInputValue(input, "hello from team acp");
    const sendButton = findButtonByText(container, "Send");
    await act(async () => {
      sendButton.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      sendButton.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      await Promise.resolve();
    });

    expect(onSendInput).toHaveBeenCalledTimes(1);
    expect(sendButton.disabled).toBe(true);

    await act(async () => {
      resolveSend?.();
      await Promise.resolve();
    });
    expect(sendButton.disabled).toBe(false);
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
          developerMode={true}
          snapshot={buildSnapshot()}
          humanActorId="user"
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
            "worker-agent": "Worker Agent",
            user: "You",
          }}
          selectedMemberId="worker-agent"
          unreadByMemberId={{ "worker-agent": 2, user: 1 }}
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
    clickElement(findButtonByText(container, "You (human)"));

    expect(onSelectMember).toHaveBeenCalledWith("leader-agent");
    expect(onSelectMember).toHaveBeenCalledWith("user");
    expect(onConversationScroll).toHaveBeenCalledTimes(1);
    expect(onJumpToBottom).toHaveBeenCalledTimes(1);
    expect(onAckMessage).toHaveBeenCalledWith(pendingMessage);
    expect(onChatDraftChange).toHaveBeenCalledWith("hello worker");
    expect(onSendChatMessage).toHaveBeenCalledTimes(2);
    expect(toPrettyJson).toHaveBeenCalledWith({ type: "status_update", done: true });
    expect(container.textContent).toContain("Leader Agent → Worker Agent");
    expect(container.textContent).toContain("Worker Agent (worker)");

    act(() => {
      root.render(
        <TeamMailboxPanel
          developerMode={true}
          mode="advanced_only"
          snapshot={buildSnapshot()}
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
            "worker-agent": "Worker Agent",
          }}
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
          developerMode={true}
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

  it("TeamMailboxPanel renders chat_message payload strings as plain text", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    act(() => {
      root.render(
        <TeamMailboxPanel
          developerMode={true}
          snapshot={buildSnapshot()}
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
            "worker-agent": "Worker Agent",
          }}
          selectedMemberId="worker-agent"
          unreadByMemberId={{}}
          onSelectMember={vi.fn()}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "worker-agent",
            inboxActorId: "worker-agent",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={vi.fn()}
          onJumpToBottom={vi.fn()}
          conversationMessages={[
            buildMailboxMessage(30, {
              from_actor_id: "worker-agent",
              to_actor_id: "leader-agent",
              status: "delivered",
              payload:
                '{"type":"chat_message","text":"mailbox string payload text","source":"team_workbench"}',
            }),
          ]}
          toPrettyJson={toPrettyJson}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={vi.fn()}
          chatDraft=""
          onChatDraftChange={vi.fn()}
          onSendChatMessage={vi.fn()}
          msgFromActorId="leader-agent"
          onMsgFromActorIdChange={vi.fn()}
          msgToActorId="worker-agent"
          onMsgToActorIdChange={vi.fn()}
          msgChannel="default"
          onMsgChannelChange={vi.fn()}
          msgTransport="local"
          onMsgTransportChange={vi.fn()}
          msgRoute="{}"
          onMsgRouteChange={vi.fn()}
          mailboxTemplateOptions={[]}
          msgTemplate=""
          onMsgTemplateChange={vi.fn()}
          onApplyMessageTemplate={vi.fn()}
          msgPayload="{}"
          onMsgPayloadChange={vi.fn()}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={vi.fn()}
          onSendMessage={vi.fn()}
          inboxActorId="worker-agent"
          onInboxActorIdChange={vi.fn()}
          inboxLimit="20"
          onInboxLimitChange={vi.fn()}
          inboxAfterId=""
          onInboxAfterIdChange={vi.fn()}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={vi.fn()}
          onRefreshInbox={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain("mailbox string payload text");
    expect(container.textContent).not.toContain('{"type":"chat_message"');
    expect(container.textContent).toContain("Worker Agent → Leader Agent");
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamMailboxPanel renders plain markdown string payloads without JSON escaping", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    act(() => {
      root.render(
        <TeamMailboxPanel
          developerMode={true}
          snapshot={buildSnapshot()}
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
            "worker-agent": "Worker Agent",
          }}
          selectedMemberId="worker-agent"
          unreadByMemberId={{}}
          onSelectMember={vi.fn()}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "worker-agent",
            inboxActorId: "worker-agent",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={vi.fn()}
          onJumpToBottom={vi.fn()}
          conversationMessages={[
            buildMailboxMessage(31, {
              from_actor_id: "worker-agent",
              to_actor_id: "leader-agent",
              status: "delivered",
              payload: "line one\n\n- line two",
            }),
          ]}
          toPrettyJson={toPrettyJson}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={vi.fn()}
          chatDraft=""
          onChatDraftChange={vi.fn()}
          onSendChatMessage={vi.fn()}
          msgFromActorId="leader-agent"
          onMsgFromActorIdChange={vi.fn()}
          msgToActorId="worker-agent"
          onMsgToActorIdChange={vi.fn()}
          msgChannel="default"
          onMsgChannelChange={vi.fn()}
          msgTransport="local"
          onMsgTransportChange={vi.fn()}
          msgRoute="{}"
          onMsgRouteChange={vi.fn()}
          mailboxTemplateOptions={[]}
          msgTemplate=""
          onMsgTemplateChange={vi.fn()}
          onApplyMessageTemplate={vi.fn()}
          msgPayload="{}"
          onMsgPayloadChange={vi.fn()}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={vi.fn()}
          onSendMessage={vi.fn()}
          inboxActorId="worker-agent"
          onInboxActorIdChange={vi.fn()}
          inboxLimit="20"
          onInboxLimitChange={vi.fn()}
          inboxAfterId=""
          onInboxAfterIdChange={vi.fn()}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={vi.fn()}
          onRefreshInbox={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain("line one");
    expect(container.textContent).toContain("line two");
    expect(container.textContent).not.toContain('"line one\\n\\n- line two"');
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamMailboxPanel renders user sub-identities as You", () => {
    act(() => {
      root.render(
        <TeamMailboxPanel
          developerMode={false}
          snapshot={buildSnapshot()}
          humanActorId="user"
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
          }}
          selectedMemberId="user"
          unreadByMemberId={{ user: 1 }}
          onSelectMember={vi.fn()}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "user:root",
            inboxActorId: "user:root",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={vi.fn()}
          onJumpToBottom={vi.fn()}
          conversationMessages={[
            buildMailboxMessage(31, {
              from_actor_id: "leader-agent",
              to_actor_id: "user:root",
              status: "delivered",
              payload: { type: "chat_message", text: "hello" },
            }),
          ]}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={vi.fn()}
          chatDraft=""
          onChatDraftChange={vi.fn()}
          onSendChatMessage={vi.fn()}
          msgFromActorId=""
          onMsgFromActorIdChange={vi.fn()}
          msgToActorId=""
          onMsgToActorIdChange={vi.fn()}
          msgChannel=""
          onMsgChannelChange={vi.fn()}
          msgTransport="local"
          onMsgTransportChange={vi.fn()}
          msgRoute=""
          onMsgRouteChange={vi.fn()}
          mailboxTemplateOptions={[]}
          msgTemplate=""
          onMsgTemplateChange={vi.fn()}
          onApplyMessageTemplate={vi.fn()}
          msgPayload="{}"
          onMsgPayloadChange={vi.fn()}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={vi.fn()}
          onSendMessage={vi.fn()}
          inboxActorId=""
          onInboxActorIdChange={vi.fn()}
          inboxLimit="20"
          onInboxLimitChange={vi.fn()}
          inboxAfterId=""
          onInboxAfterIdChange={vi.fn()}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={vi.fn()}
          onRefreshInbox={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain("Leader Agent → You");
  });

  it("TeamMailboxPanel hides raw mailbox tools when developer mode is off", () => {
    act(() => {
      root.render(
        <TeamMailboxPanel
          developerMode={false}
          mode="advanced_only"
          snapshot={buildSnapshot()}
          displayNameByActorId={{
            "leader-agent": "Leader Agent",
            "worker-agent": "Worker Agent",
          }}
          selectedMemberId="worker-agent"
          unreadByMemberId={{}}
          onSelectMember={vi.fn()}
          chatActors={{
            fromActorId: "leader-agent",
            toActorId: "worker-agent",
            inboxActorId: "worker-agent",
          }}
          chatStickToBottom={true}
          chatMessagesRef={React.createRef<HTMLUListElement>()}
          onConversationScroll={vi.fn()}
          onJumpToBottom={vi.fn()}
          conversationMessages={[]}
          toPrettyJson={(value) => JSON.stringify(value)}
          formatTs={(ts) => `ts-${String(ts)}`}
          busy={null}
          onAckMessage={vi.fn()}
          chatDraft=""
          onChatDraftChange={vi.fn()}
          onSendChatMessage={vi.fn()}
          msgFromActorId="leader-agent"
          onMsgFromActorIdChange={vi.fn()}
          msgToActorId="worker-agent"
          onMsgToActorIdChange={vi.fn()}
          msgChannel="default"
          onMsgChannelChange={vi.fn()}
          msgTransport="local"
          onMsgTransportChange={vi.fn()}
          msgRoute="{}"
          onMsgRouteChange={vi.fn()}
          mailboxTemplateOptions={[]}
          msgTemplate=""
          onMsgTemplateChange={vi.fn()}
          onApplyMessageTemplate={vi.fn()}
          msgPayload="{}"
          onMsgPayloadChange={vi.fn()}
          msgIdempotencyKey=""
          onMsgIdempotencyKeyChange={vi.fn()}
          onSendMessage={vi.fn()}
          inboxActorId="worker-agent"
          onInboxActorIdChange={vi.fn()}
          inboxLimit="20"
          onInboxLimitChange={vi.fn()}
          inboxAfterId=""
          onInboxAfterIdChange={vi.fn()}
          inboxIncludeDelivered={false}
          onInboxIncludeDeliveredChange={vi.fn()}
          onRefreshInbox={vi.fn()}
        />
      );
    });

    expect(container.textContent).toContain(
      "Enable Developer Mode in Admin to access raw mailbox tools."
    );
    expect(container.textContent).not.toContain("Advanced mailbox controls");
    expect(container.textContent).not.toContain("from_actor_id");
  });
});
