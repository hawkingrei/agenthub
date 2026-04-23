// @vitest-environment jsdom
import { MantineProvider as CoreMantineProvider } from "@mantine/core";
import React, { act } from "react";
import { createRoot, Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  api,
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
import * as mailboxHelpers from "./team/mailbox_helpers";
import {
  TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS,
  TEAM_SIDEBAR_SECTION_TOGGLE_CLASS,
} from "../ui/tailwind_classes";
import {
  installReactDomTestGlobals,
  renderWithMantine,
  required,
} from "../test_utils/react_test_helpers";

installReactDomTestGlobals();

function MantineProvider({ children }: { children: React.ReactNode }) {
  return <CoreMantineProvider env="test">{children}</CoreMantineProvider>;
}

function clickElement(element: Element | null): void {
  const node = required(element, "element not found");
  act(() => {
    node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

async function openDebugTabAndWait(container: HTMLElement): Promise<void> {
  clickElement(findButtonByText(container, "Inspect"));
  await act(async () => {
    await vi.dynamicImportSettled();
  });
}

function clickMenuTrigger(element: Element | null): void {
  const node = required(element, "element not found");
  act(() => {
    if (typeof PointerEvent !== "undefined") {
      node.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, cancelable: true }));
      node.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, cancelable: true }));
    }
    node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
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

function queryButtonByAriaLabel(
  container: HTMLElement,
  label: string
): HTMLButtonElement | null {
  const normalized = label.toLowerCase();
  return (
    (Array.from(container.querySelectorAll("button")).find((candidate) =>
      candidate.getAttribute("aria-label")?.toLowerCase().includes(normalized)
    ) as HTMLButtonElement | undefined) ?? null
  );
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

async function waitForCondition(
  predicate: () => boolean,
  attempts = 40
): Promise<void> {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if (predicate()) {
      return;
    }
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 0));
    });
  }
  throw new Error("condition not met before timeout");
}

async function openTaskDetailModal(
  container: HTMLElement,
  taskTitle: string
): Promise<void> {
  clickElement(findButtonByText(container, taskTitle));
  await waitForCondition(() => document.body.querySelector('[role="dialog"]') !== null);
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
    status:
      | "open"
      | "in_progress"
      | "waiting"
      | "in_review"
      | "completed"
      | "canceled";
    assigned_member_id: string | null;
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
    assigned_member_id: overrides.assigned_member_id ?? null,
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

  it("TeamSidebar renders subject rail and triggers navigation callbacks", async () => {
    const onRefreshTeams = vi.fn();
    const onOpenCreateTeam = vi.fn();
    const onSelectTeam = vi.fn();
    const onSelectChannel = vi.fn();
    const onSelectKanban = vi.fn();
    const onSelectAgentTab = vi.fn();
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
            onSelectChannel={onSelectChannel}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
          />
        </MantineProvider>
      );
    });

    expect(container.querySelector('[data-team-surface="sidebar"]')).not.toBeNull();
    clickElement(findButtonByAriaLabel(container, "Refresh teams"));
    clickMenuTrigger(findButtonByAriaLabel(container, "Open team actions"));
    await waitForCondition(() => document.body.textContent?.includes("Create Team") ?? false);
    clickElement(findInteractiveByText(document.body, "Create Team"));
    const filterInput = required(
      container.querySelector("input[aria-label='Search teams']"),
      "team filter input missing"
    ) as HTMLInputElement;
    expect(container.textContent).not.toContain("draft_team=alpha");
    changeInputValue(filterInput, "team-2");
    expect(container.textContent).toContain("Team Two");
    clickElement(findButtonByAriaLabel(container, "Clear filter"));
    clickMenuTrigger(findButtonByAriaLabel(container, "Open team actions"));
    await waitForCondition(
      () => document.body.textContent?.includes("Show Team Details") ?? false
    );
    clickElement(findInteractiveByText(document.body, "Show Team Details"));
    expect(container.textContent).toContain("draft_team=alpha");
    expect(container.textContent).toContain("leader=leader-agent");
    expect(container.textContent).toContain("workers=2");
    clickElement(findButtonByText(container, "Team Two"));
    expect(container.querySelector("input[aria-label='Search teams']")).not.toBeNull();
    expect(container.textContent).toContain("Team One");
    clickElement(findButtonByText(container, "Kanban"));
    clickElement(findButtonByAriaLabel(container, "Toggle agents section"));
    expect(container.textContent).not.toContain("Worker Agent");
    clickElement(findButtonByAriaLabel(container, "Toggle agents section"));
    expect(container.textContent).toContain("Worker Agent");
    clickElement(findButtonByText(container, "# all"));
    clickElement(findButtonByText(container, "Worker Agent"));

    expect(onRefreshTeams).toHaveBeenCalledTimes(1);
    expect(onOpenCreateTeam).toHaveBeenCalledTimes(1);
    expect(onSelectTeam).toHaveBeenCalledWith("team-2");
    expect(onSelectChannel).toHaveBeenCalledWith("all");
    expect(onSelectKanban).toHaveBeenCalledTimes(1);
    expect(onSelectAgentTab).toHaveBeenCalledWith("worker-agent", "agent_acp");
    expect(container.textContent).toContain("Teams");
    expect(container.textContent).toContain("Kanban");
    expect(container.textContent).toContain("Agents");
    expect(container.textContent).toContain("Channels");
    expect(container.textContent).toContain("# all");
    expect(container.textContent).toContain(
      "Shared coordination lane for requests, updates, and cross-cutting discussion."
    );
    expect(findButtonByText(container, "Team Two").className).toContain("rounded-md");
    expect(findButtonByText(container, "Team Two").className).toContain("px-2");
    const kanbanButton = findButtonByText(container, "Kanban");
    const channelButton = findButtonByText(container, "# all");
    expect(kanbanButton.className).toContain("rounded-md");
    expect(kanbanButton.className).toContain("px-2");
    expect(
      Boolean(channelButton.compareDocumentPosition(kanbanButton) & Node.DOCUMENT_POSITION_FOLLOWING)
    ).toBe(true);
    expect(container.textContent).toContain("Leader Agent");
    expect(container.textContent).toContain("Worker Agent");
    expect(container.textContent).toContain("planning handoff");
    expect(container.textContent).toContain("collecting evidence");
    expect(container.textContent).toContain("Working");
    expect(container.textContent).not.toContain("id leader-agent");
    expect(container.textContent).not.toContain("id worker-agent");
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
            onSelectChannel={onSelectChannel}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Open team actions"));
    expect(container.textContent).not.toContain("Show Team Details");
    expect(container.textContent).not.toContain("team-1");

    expect(container.textContent).toContain("Teams");
    expect(container.textContent).toContain("# all");
    expect(container.textContent).not.toContain("Execution Runs");
    expect(container.textContent).not.toContain("Advanced");

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
            onSelectChannel={onSelectChannel}
            onSelectKanban={onSelectKanban}
            onSelectAgentTab={onSelectAgentTab}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByText(container, "# all"));
    expect(onSelectChannel).toHaveBeenCalledTimes(2);

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
            onSelectChannel={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
          />
        </MantineProvider>
      );
    });

    const unmatchedFilterInput = required(
      container.querySelector("input[aria-label='Search teams']"),
      "team filter input missing"
    ) as HTMLInputElement;
    changeInputValue(unmatchedFilterInput, "missing-team");
    expect(container.textContent).toContain("No results found.");

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
            onSelectChannel={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Create a team to begin.");
    expect(container.querySelector("input[aria-label='Filter teams']")).toBeNull();
    clickElement(findButtonByText(container, "Create Team"));
    expect(noTeamsCreate).toHaveBeenCalledTimes(1);
  });

  it("TeamSidebar exposes create and delete channel controls for non-default lanes", async () => {
    const onCreateChannel = vi.fn().mockResolvedValue(undefined);
    const onDeleteChannel = vi.fn().mockResolvedValue(undefined);
    const teamOne = buildTeam();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            showTeamSelector={false}
            developerMode={true}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={() => {}}
            draftTeamName=""
            leaderMemberId="leader-agent"
            configuredWorkerCount={1}
            teams={[teamOne]}
            selectedTeam={teamOne}
            selectedTeamId={teamOne.id}
            selectedTeamHasConfiguredMembers={true}
            teamMemberSummaryByTeamId={new Map()}
            memberLiveStates={[buildMemberLiveState()]}
            channelItems={[
              {
                id: "all",
                label: "# all",
                description: "Shared coordination lane",
              },
              {
                id: "review",
                label: "# review",
                description: "Review lane",
              },
            ]}
            selectedChannelId="review"
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={() => {}}
            onSelectChannel={() => {}}
            onCreateChannel={onCreateChannel}
            onDeleteChannel={onDeleteChannel}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Create channel"));
    const channelIdInput = required(
      container.querySelector("input[aria-label='Channel ID']"),
      "channel id input missing"
    ) as HTMLInputElement;
    const descriptionInput = required(
      container.querySelector("input[aria-label='Channel Description']"),
      "channel description input missing"
    ) as HTMLInputElement;
    changeInputValue(channelIdInput, " research ");
    changeInputValue(descriptionInput, " Investigation lane ");
    clickElement(findButtonByText(container, "Create channel"));
    await waitForCondition(() => onCreateChannel.mock.calls.length === 1);
    expect(onCreateChannel).toHaveBeenCalledWith({
      channelId: "research",
      description: "Investigation lane",
    });

    expect(queryButtonByAriaLabel(container, "Delete channel all")).toBeNull();
    clickElement(findButtonByAriaLabel(container, "Delete channel review"));
    expect(onDeleteChannel).toHaveBeenCalledWith("review");
  });

  it("TeamSidebar uses the team name as a team switcher and keeps controls in a separate menu", async () => {
    const teamOne = buildTeam({
      description: "Triage TiDB issues and coordinate the fuzzing backlog.",
    });
    const teamTwo = buildTeam({ id: "team-2", name: "Team Two" });
    const onBackToSelector = vi.fn();
    const onSelectTeam = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            showTeamSelector={false}
            developerMode={true}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={() => {}}
            draftTeamName=""
            leaderMemberId="leader-agent"
            configuredWorkerCount={2}
            teams={[teamOne, teamTwo]}
            selectedTeam={teamOne}
            selectedTeamId={teamOne.id}
            selectedTeamRuntimeStatus={{
              label: "Team running",
              online: 3,
              total: 3,
              status: "running",
            }}
            selectedTeamMemberCount={3}
            selectedTeamHasConfiguredMembers={true}
            teamMemberSummaryByTeamId={new Map()}
            memberLiveStates={[
              buildMemberLiveState(),
              buildMemberLiveState({
                member_id: "worker-agent",
                role: "worker",
                agent_name: "Worker Agent",
              }),
              buildMemberLiveState({
                member_id: "worker-agent-2",
                role: "worker",
                agent_name: "Worker Agent 2",
              }),
            ]}
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={onSelectTeam}
            onBackToSelector={onBackToSelector}
            onSelectChannel={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
            onOpenTeamMemberForge={() => {}}
            onStartTeamRuntime={() => {}}
            onStopTeamRuntime={() => {}}
          />
        </MantineProvider>
      );
    });

    clickMenuTrigger(findButtonByAriaLabel(container, "Switch teams from Team One"));
    await waitForCondition(() => document.body.textContent?.includes("All Teams") ?? false);
    clickElement(findInteractiveByText(document.body, "All Teams"));
    expect(onBackToSelector).toHaveBeenCalledTimes(1);
    clickMenuTrigger(findButtonByAriaLabel(container, "Switch teams from Team One"));
    await waitForCondition(() => document.body.textContent?.includes("Team Two") ?? false);
    clickElement(findInteractiveByText(document.body, "Team Two"));
    expect(onSelectTeam).toHaveBeenCalledWith("team-2");
    expect(findButtonByAriaLabel(container, "Open controls for Team One")).not.toBeNull();
    expect(container.textContent).not.toContain("Team running · 3/3 online");
    expect(container.textContent).not.toContain("Triage TiDB issues and coordinate the fuzzing backlog.");
    expect(container.textContent).toContain("Team One");
  });

  it("TeamSidebar detail menu exposes switch-team and runtime actions", async () => {
    const teamOne = buildTeam({ id: "team-1", name: "Team One" });
    const teamTwo = buildTeam({ id: "team-2", name: "Team Two" });
    const onSelectTeam = vi.fn();
    const onOpenTeamMemberForge = vi.fn();
    const onStartTeamRuntime = vi.fn();
    const onStopTeamRuntime = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamSidebar
            showTeamSelector={false}
            developerMode={true}
            busy={null}
            onRefreshTeams={() => {}}
            onOpenCreateTeam={() => {}}
            draftTeamName=""
            leaderMemberId="leader-agent"
            configuredWorkerCount={1}
            teams={[teamOne, teamTwo]}
            selectedTeam={teamOne}
            selectedTeamId={teamOne.id}
            selectedTeamRuntimeStatus={{
              label: "Team running",
              online: 1,
              total: 1,
              status: "running",
            }}
            selectedTeamHasConfiguredMembers={true}
            teamMemberSummaryByTeamId={new Map()}
            memberLiveStates={[buildMemberLiveState()]}
            focusedAgentMemberId=""
            tab="conversation"
            onSelectTeam={onSelectTeam}
            onSelectChannel={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
            onOpenTeamMemberForge={onOpenTeamMemberForge}
            onStartTeamRuntime={onStartTeamRuntime}
            onStopTeamRuntime={onStopTeamRuntime}
          />
        </MantineProvider>
      );
    });

    clickMenuTrigger(findButtonByAriaLabel(container, "Open controls for Team One"));
    await waitForCondition(() => document.body.textContent?.includes("Switch team") ?? false);
    expect(document.body.textContent).toContain("Team ID");
    clickElement(findInteractiveByText(document.body, "Team Two"));
    clickMenuTrigger(findButtonByAriaLabel(container, "Open controls for Team One"));
    await waitForCondition(() => document.body.textContent?.includes("Add Agent") ?? false);
    clickElement(findInteractiveByText(document.body, "Add Agent"));
    clickMenuTrigger(findButtonByAriaLabel(container, "Open controls for Team One"));
    await waitForCondition(() => document.body.textContent?.includes("Stop Team") ?? false);
    clickElement(findInteractiveByText(document.body, "Stop Team"));

    expect(onSelectTeam).toHaveBeenCalledWith("team-2");
    expect(onOpenTeamMemberForge).toHaveBeenCalledTimes(1);
    expect(onStopTeamRuntime).toHaveBeenCalledTimes(1);
    expect(onStartTeamRuntime).not.toHaveBeenCalled();
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
    clickElement(findButtonByText(container, "Start Execution Run"));
    clickElement(findButtonByAriaLabel(container, "Refresh execution runs"));
    clickElement(required(container.querySelector(".teams-run-list .team-item"), "run list item missing"));
    clickElement(findButtonByText(container, "Load More"));

    expect(onDeleteTeam).toHaveBeenCalledTimes(1);
    expect(onStartRun).toHaveBeenCalledTimes(1);
    expect(onRunStatusFilterChange).toHaveBeenCalledWith("working");
    expect(onRefreshRuns).toHaveBeenCalledTimes(1);
    expect(onActiveRunChange).toHaveBeenCalledWith("run-1");
    expect(onLoadMoreRuns).toHaveBeenCalledTimes(1);
    expect(
      findButtonByAriaLabel(container, "Start execution run").getAttribute("title")
    ).toBe("Start a new execution run");

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
            runsHasMore={false}
            selectedTeamId={null}
            onLoadMoreRuns={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Active run `run-hidden` is hidden by filter `completed`.");
    expect(container.textContent).toContain(
      "No execution runs loaded yet. Use Debug → Run Ops to create or load runs."
    );
    expect(container.textContent).toContain(
      "Concrete execution runs and replay partitions for this team."
    );
    expect(
      findButtonByAriaLabel(container, "Start execution run").getAttribute("title")
    ).toBe("Add at least one agent before starting the team runtime or a run.");
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
            onSelectChannel={() => {}}
            onSelectKanban={() => {}}
            onSelectAgentTab={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Detail Team");
    expect(container.textContent).not.toContain("Browse this team's channels, members, and operations.");
    expect(container.textContent).not.toContain("Team Selector");
    expect(container.querySelector("input[aria-label='Filter teams']")).toBeNull();
    expect(container.textContent).not.toContain("Teams");
    expect(container.textContent).not.toContain("Create Team");
    expect(container.textContent).not.toContain("Shared team thread");
    expect(container.textContent).not.toContain("Task board");
    expect(container.textContent).toContain("Channels");
    expect(container.textContent).toContain("Agents");
  });

  it("TeamSidebar keeps visible keyboard focus treatments on section toggles and nav rows", () => {
    expect(TEAM_SIDEBAR_SECTION_TOGGLE_CLASS).toContain("focus-visible:ring-2");
    expect(TEAM_SIDEBAR_SECTION_TOGGLE_CLASS).toContain("focus-visible:ring-offset-1");
    expect(TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS).toContain("focus-visible:ring-2");
    expect(TEAM_SIDEBAR_NAV_ITEM_ACTIVE_CLASS).toContain("focus-visible:ring-offset-1");
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
    expect(container.textContent).toContain("Start Execution Run");
    expect(container.textContent).not.toContain("Create Run");
    expect(container.querySelector('textarea[aria-label="Run input JSON"]')).toBeNull();
  });

  it("TeamTabsBar renders product tabs and switches selected tab", () => {
    const onTabChange = vi.fn();
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTabsBar tab="runs" onTabChange={onTabChange} />
        </MantineProvider>
      );
    });

    expect(container.querySelector('[data-team-surface="workflow-tabs"]')).not.toBeNull();
    expect(container.textContent).toContain("Execution Runs");
    expect(container.textContent).toContain("Conversation");
    expect(container.textContent).toContain("Agent ACP");
    expect(container.textContent).toContain("Debug");
    expect(
      Array.from(container.querySelectorAll('[data-team-surface="workflow-tabs"] button')).every(
        (button) => button.getAttribute("type") === "button"
      )
    ).toBe(true);

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
        <MantineProvider>
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
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Active Execution Run");
    expect(container.textContent).toContain("Execution status:");
    expect(container.textContent).toContain("run-1");
    expect(container.textContent).toContain("ctx-1");
    expect(
      findButtonByAriaLabel(container, "Refresh active execution run").getAttribute("title")
    ).toBe("Refresh active execution run");
    clickElement(findButtonByAriaLabel(container, "Refresh active execution run"));
    clickElement(findButtonByText(container, "Cancel Execution Run"));
    clickElement(findButtonByText(container, "Resume Execution Run"));
    clickElement(findButtonByText(container, "Restart Execution Run"));
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
      root.render(
        <MantineProvider>
          <TeamStepsPanel {...baseProps} />
        </MantineProvider>
      );
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
      root.render(
        <MantineProvider>
          <TeamStepsPanel {...baseProps} stepAction="complete" />
        </MantineProvider>
      );
    });
    changeInputValue(
      required(
        container.querySelectorAll(".teams-step-panel textarea")[1] as HTMLTextAreaElement | null,
        "complete output textarea missing"
      ),
      "{\"ok\":true}"
    );

    act(() => {
      root.render(
        <MantineProvider>
          <TeamStepsPanel {...baseProps} stepAction="fail" />
        </MantineProvider>
      );
    });
    changeInputValue(
      required(container.querySelector('input[placeholder="error_text"]') as HTMLInputElement | null, "error_text input missing"),
      "failed"
    );

    act(() => {
      root.render(
        <MantineProvider>
          <TeamStepsPanel {...baseProps} stepAction="input_required" />
        </MantineProvider>
      );
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
      root.render(
        <MantineProvider>
          <TeamStepsPanel {...baseProps} stepAction="resume" />
        </MantineProvider>
      );
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
        <MantineProvider>
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
        </MantineProvider>
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
        <MantineProvider>
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
        </MantineProvider>
      );
    });
    expect(findButtonByText(container, "Submit Step")).toBeDefined();
    expect(findButtonByText(container, "Apply Step Action")).toBeDefined();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamStepsPanel
            developerMode={true}
            mode="list_only"
            steps={[]}
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
        </MantineProvider>
      );
    });
    expect(container.textContent).toContain("No steps yet");
    expect(container.textContent).toContain(
      "Start a run or open Debug -> Step Ops to seed execution steps."
    );

    act(() => {
      root.render(
        <MantineProvider>
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
        </MantineProvider>
      );
    });
    expect(container.textContent).toContain("Step controls are available in Developer Mode.");
    expect(container.querySelector(".teams-step-list")).not.toBeNull();
  });

  it("TeamStepsPanel renders key-value step metadata and stable notice styling", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamStepsPanel
            developerMode={true}
            mode="list_only"
            steps={[
              buildStep({
                id: "step-2",
                step_key: "dispatch",
                member_id: "worker-2",
                attempt: 3,
                depends_on: ["plan", "seed"],
                runtime_handle_id: "runtime-7",
                error_text: "needs retry",
              }),
            ]}
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
        </MantineProvider>
      );
    });

    const warningNotice = required(
      container.querySelector(".mb-3.text-ui-sm"),
      "warning notice missing"
    );
    expect(warningNotice.className).toContain("border-state-warning-border");
    expect(warningNotice.className).toContain("bg-state-warning-bg/60");
    expect(warningNotice.className).toContain("mb-3");
    expect(container.textContent).toContain("member_id");
    expect(container.textContent).toContain("worker-2");
    expect(container.textContent).toContain("attempt");
    expect(container.textContent).toContain("3");
    expect(container.textContent).toContain("depends_on");
    expect(container.textContent).toContain("plan, seed");
    expect(container.textContent).toContain("runtime_handle_id");
    expect(container.textContent).toContain("runtime-7");
    expect(container.textContent).toContain("error_text");
    expect(container.textContent).toContain("needs retry");
    const stepItem = required(
      container.querySelector(".teams-step-list li .min-h-0.rounded-xl"),
      "step item surface missing"
    );
    expect(stepItem.className).toContain("p-2");
    expect(stepItem.className).toContain("sm:p-2");
    expect(stepItem.className).not.toContain("rounded-lg");
  });

  it("TeamEventsPanel supports auto-refresh toggle and load older actions", () => {
    const onEventsAutoRefreshChange = vi.fn();
    const onRefreshEvents = vi.fn();
    const onLoadOlderEvents = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
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
        </MantineProvider>
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
        <MantineProvider>
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
        </MantineProvider>
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
        <MantineProvider>
          <TeamOverviewPanel
            snapshot={buildSnapshot()}
            snapshotLoading={false}
            onRefreshSnapshot={onRefreshSnapshot}
            selectedMemberId="leader-agent"
            onOpenMailboxForMember={onOpenMailboxForMember}
          />
        </MantineProvider>
      );
    });

    clickElement(findButtonByAriaLabel(container, "Refresh snapshot"));
    clickElement(required(container.querySelectorAll(".teams-member-list .team-item")[1], "member button missing"));

    expect(onRefreshSnapshot).toHaveBeenCalledTimes(1);
    expect(onOpenMailboxForMember).toHaveBeenCalledWith("worker-agent");
    expect(container.textContent).toContain("Cold Start Playbook");
    expect(container.textContent).toContain("Leader startup");
    expect(container.textContent).toContain("Worker startup");
    expect(container.querySelector(".teams-overview-meta")).not.toBeNull();
    expect(container.querySelector(".teams-member-list")).not.toBeNull();
    expect(container.innerHTML).toContain("min-w-0 flex-1 break-words whitespace-normal");

    act(() => {
      root.render(
        <MantineProvider>
          <TeamOverviewPanel
            snapshot={null}
            snapshotLoading={false}
            onRefreshSnapshot={() => {}}
            selectedMemberId=""
            onOpenMailboxForMember={() => {}}
          />
        </MantineProvider>
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
        <MantineProvider>
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
        </MantineProvider>
      );
    });

    expect(container.querySelector('[data-team-panel="member-console"]')).not.toBeNull();
    changeSelectValue(
      required(container.querySelector("select") as HTMLSelectElement | null, "member select missing"),
      "worker-agent"
    );
    clickElement(findButtonByAriaLabel(container, "Refresh member console"));

    act(() => {
      root.render(
        <MantineProvider>
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
        </MantineProvider>
      );
    });

    clickElement(findButtonByText(container, "Load Older"));
    expect(container.textContent).toContain("Selected member has no associated session yet.");

    act(() => {
      root.render(
        <MantineProvider>
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
        </MantineProvider>
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
        <MantineProvider>
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
        </MantineProvider>
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
    const onMessageDraftChange = vi.fn();
    const onSendMessage = vi.fn();
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    function TeamTaskPanelHarness() {
      const [draft, setDraft] = React.useState("please continue @Worker Agent");
      return (
        <TeamTaskPanel
          developerMode={true}
          messageDraft={draft}
          onMessageDraftChange={(value) => {
            onMessageDraftChange(value);
            setDraft(value);
          }}
          onSendMessage={onSendMessage}
          messages={[
            buildTaskMessage(1),
            buildTaskMessage(2, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "leader reply visible in all" },
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

    renderWithMantine(root, <TeamTaskPanelHarness />);

    expect(container.querySelector('[data-team-surface="conversation"]')).not.toBeNull();
    expect(queryButtonByAriaLabel(container, "Toggle thread options")).toBeNull();
    expect(queryButtonByText(container, "Refresh Channel")).toBeNull();
    expect(queryButtonByText(container, "Refresh Thread")).toBeNull();
    const draftTextarea = required(
      container.querySelector(
        'textarea[placeholder="Message #all"]'
      ) as HTMLTextAreaElement | null,
      "draft textarea missing"
    );
    expect(draftTextarea.id).toBe("team-task-panel-message");
    expect(draftTextarea.name).toBe("team_task_message");
    changeInputValue(draftTextarea, "please continue @Worker Agent and review");
    clickElement(findButtonByText(container, "Send"));

    expect(onSendMessage).toHaveBeenCalledTimes(1);
    expect(onSendMessage).toHaveBeenCalledWith({
      text: "please continue <at>worker-agent</at> and review",
      mentionActorIds: ["worker-agent"],
    });
    expect(onMessageDraftChange).toHaveBeenCalledWith("please continue @Worker Agent and review");
    expect(toPrettyJson).not.toHaveBeenCalled();
    expect(container.textContent).not.toContain("(task-1)");
    expect(container.textContent).not.toContain("conversation_id=task-1");
    expect(container.querySelector("h3")).toBeNull();
    expect(container.textContent).not.toContain(
      "General channel for shared planning, requests, and broadcast coordination."
    );
    expect(container.textContent).toContain(
      "@name to reply · Enter to send"
    );
    expect(container.textContent).not.toContain("status_update");
    expect(container.textContent).not.toContain("work:working");
    expect(container.textContent).not.toContain("agent:working");
    const detailButtons = Array.from(container.querySelectorAll("button")).filter((candidate) =>
      candidate.textContent?.includes("Details")
    );
    clickElement(detailButtons[1] ?? null);
    expect(container.textContent).toContain("work");
    expect(container.textContent).toContain("working/working");
    expect(container.textContent).toContain("agent");
    expect(container.textContent).toContain("running");
  });

  it("TeamTaskPanel keeps thread replies out of the main channel timeline", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={false}
        tasksLoading={false}
        onRefreshTasks={vi.fn()}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(1, {
            from_actor_id: "leader-agent",
            to_actor_id: null,
            route: "group_chat",
            payload: { type: "chat_message", text: "Channel root message stays visible." },
          }),
          buildTaskMessage(2, {
            from_actor_id: "worker-agent",
            to_actor_id: null,
            route: "team_thread_reply",
            payload: {
              type: "chat_message",
              text: "Threaded follow-up should stay in the thread pane only.",
              thread_root_message_id: 1,
            },
          }),
        ]}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={(value) => JSON.stringify(value)}
      />
    );

    expect(container.textContent).toContain("Channel root message stays visible.");
    expect(container.textContent).not.toContain(
      "Threaded follow-up should stay in the thread pane only."
    );
  });

  it("TeamTaskPanel keeps the channel body in a dedicated flex shell above the composer", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={true}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(1, {
            from_actor_id: "leader-agent",
            to_actor_id: null,
            route: "group_chat",
            payload: { type: "chat_message", text: "Keep the composer docked." },
          }),
        ]}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={toPrettyJson}
      />
    );

    const rootCard = required(
      container.querySelector('[data-team-surface="conversation"]') as HTMLDivElement | null,
      "team task panel root missing"
    );
    expect(rootCard.classList.contains("flex")).toBe(true);
    expect(rootCard.classList.contains("flex-col")).toBe(true);
    expect(rootCard.classList.contains("flex-1")).toBe(true);
    expect(rootCard.classList.contains("overflow-hidden")).toBe(true);

    const bodyShell = required(
      container.querySelector('[data-team-channel-body="true"]') as HTMLDivElement | null,
      "team channel body shell missing"
    );
    expect(bodyShell.classList.contains("min-h-0")).toBe(true);
    expect(bodyShell.classList.contains("flex")).toBe(true);
    expect(bodyShell.classList.contains("flex-1")).toBe(true);
    expect(bodyShell.classList.contains("flex-col")).toBe(true);
    expect(bodyShell.classList.contains("overflow-hidden")).toBe(true);
    expect(bodyShell.classList.contains("px-2")).toBe(true);

    const composer = required(
      container.querySelector('[data-team-channel-composer="true"]') as HTMLDivElement | null,
      "team channel composer missing"
    );
    expect(composer.classList.contains("shrink-0")).toBe(true);
    expect(composer.classList.contains("px-2")).toBe(true);

    const scrollNode = required(
      container.querySelector('[data-team-channel-scroll="true"]') as HTMLDivElement | null,
      "team channel scroll container missing"
    );
    expect(scrollNode.classList.contains("min-h-0")).toBe(true);
    expect(scrollNode.classList.contains("flex-1")).toBe(true);
    expect(scrollNode.classList.contains("overflow-y-auto")).toBe(true);

    expect(rootCard.lastElementChild).toBe(composer);
    expect(bodyShell.nextElementSibling).toBe(composer);
  });

  it("TeamTaskPanel renders mention suggestions with shared option rows", () => {
    function TeamTaskPanelHarness() {
      const [draft, setDraft] = React.useState("");
      return (
        <TeamTaskPanel
          developerMode={false}
          messageDraft={draft}
          onMessageDraftChange={setDraft}
          onSendMessage={vi.fn()}
          messages={[]}
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "Worker Agent",
              lifecycle_status: "running",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "shipping patch",
            },
          ]}
          memberIds={["worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    }

    renderWithMantine(root, <TeamTaskPanelHarness />);

    const draftTextarea = required(
      container.querySelector("#team-task-panel-message") as HTMLTextAreaElement | null,
      "draft textarea missing"
    );
    changeInputValue(draftTextarea, "@W");

    const option = required(
      container.querySelector('[data-team-mention-option="worker-agent"]') as HTMLButtonElement | null,
      "mention option missing"
    );
    expect(option.textContent).toContain("Worker Agent");
    expect(option.textContent).toContain("@Worker Agent");
    expect(option.className).toContain("flex w-full items-center justify-between");
  });

  it("TeamTaskPanel sends on Enter and applies mention selection on mouse down", () => {
    const onSendMessage = vi.fn();

    function TeamTaskPanelHarness() {
      const [draft, setDraft] = React.useState("");
      return (
        <TeamTaskPanel
          developerMode={false}
          messageDraft={draft}
          onMessageDraftChange={setDraft}
          onSendMessage={onSendMessage}
          messages={[]}
          memberLiveStates={[
            buildMemberLiveState({
              member_id: "worker-agent",
              role: "worker",
              agent_name: "Worker Agent",
            }),
          ]}
          memberIds={["worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    }

    renderWithMantine(root, <TeamTaskPanelHarness />);

    const draftTextarea = required(
      container.querySelector("#team-task-panel-message") as HTMLTextAreaElement | null,
      "draft textarea missing"
    );
    changeInputValue(draftTextarea, "@W");

    const option = required(
      container.querySelector('[data-team-mention-option="worker-agent"]') as HTMLButtonElement | null,
      "mention option missing"
    );
    act(() => {
      option.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    });

    expect(draftTextarea.value).toContain("@Worker Agent");

    act(() => {
      draftTextarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
      );
    });

    expect(onSendMessage).toHaveBeenCalledWith({
      text: "<at>worker-agent</at>",
      mentionActorIds: ["worker-agent"],
    });
  });

  it("TeamTaskPanel supports IME-aware mention keyboard navigation", async () => {
    const onSendMessage = vi.fn();

    function TeamTaskPanelHarness() {
      const [draft, setDraft] = React.useState("");
      return (
        <TeamTaskPanel
          developerMode={false}
          messageDraft={draft}
          onMessageDraftChange={setDraft}
          onSendMessage={onSendMessage}
          messages={[]}
          memberLiveStates={[
            buildMemberLiveState({
              member_id: "worker-agent",
              role: "worker",
              agent_name: "Worker Agent",
            }),
            buildMemberLiveState({
              member_id: "reviewer-agent",
              role: "worker",
              agent_name: "Reviewer Agent",
            }),
          ]}
          memberIds={["worker-agent", "reviewer-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
      );
    }

    renderWithMantine(root, <TeamTaskPanelHarness />);

    const draftTextarea = required(
      container.querySelector("#team-task-panel-message") as HTMLTextAreaElement | null,
      "draft textarea missing"
    );

    act(() => {
      draftTextarea.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
      draftTextarea.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true }));
    });

    changeInputValue(draftTextarea, "@");
    expect(container.querySelectorAll("[data-team-mention-option]")).toHaveLength(2);

    act(() => {
      draftTextarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true, cancelable: true })
      );
      draftTextarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true })
      );
      draftTextarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
      );
    });
    expect(container.querySelector("[data-team-mention-option]")).toBeNull();

    changeInputValue(draftTextarea, "@W");
    act(() => {
      draftTextarea.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true })
      );
      draftTextarea.dispatchEvent(new FocusEvent("blur", { bubbles: true }));
    });
    expect(onSendMessage).not.toHaveBeenCalled();
  });

  it("TeamTaskPanel renders canonical agent replies already persisted in shared thread", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={true}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
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

    expect(container.textContent).toContain("hello team");
    expect(container.textContent).toContain("leader reply visible in all");
    expect(container.textContent).not.toContain("Pending delivery");
    expect(container.textContent).toContain("You");
    expect(container.textContent).toContain("LeaderAgent");
    const progressbars = Array.from(container.querySelectorAll('[role="progressbar"]'));
    expect(progressbars).toHaveLength(1);
    expect(progressbars[0]?.getAttribute("aria-valuenow")).toBe("2");
    expect(progressbars[0]?.getAttribute("aria-valuemax")).toBe("2");
    const activityKinds = Array.from(
      container.querySelectorAll("[data-activity-author-kind]")
    ).map((node) => node.getAttribute("data-activity-author-kind"));
    expect(activityKinds).toEqual(["human", "agent"]);
  });

  it("TeamTaskPanel renders pending delivery state before any member read receipts arrive", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "queued update" },
            }),
          ]}
          seenByMessageId={{}}
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
              current_work: "broadcasting update",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
    );

    expect(container.textContent).toContain("queued update");
    const pendingButton = container.querySelector('button[aria-label="Receipt pending"]');
    expect(pendingButton).not.toBeNull();
    expect(pendingButton?.getAttribute("title")).toBe("Receipt pending");
    expect(container.querySelector('[role="progressbar"]')).toBeNull();
  });

  it("TeamTaskPanel covers thread loading state and partial read progress chips", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={false}
        tasksLoading={false}
        onRefreshTasks={vi.fn()}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[]}
        humanActorId="user"
        memberLiveStates={[
          buildMemberLiveState({
            member_id: "leader-agent",
            role: "leader",
            agent_name: "Leader Agent",
            current_work: "replying in thread",
          }),
          buildMemberLiveState({
            member_id: "worker-agent",
            role: "worker",
            agent_name: "Worker Agent",
            current_work: "reading shared updates",
          }),
          buildMemberLiveState({
            member_id: "reviewer-agent",
            role: "worker",
            agent_name: "Reviewer Agent",
            current_work: "waiting for inbox",
          }),
        ]}
        memberIds={["leader-agent", "worker-agent", "reviewer-agent"]}
        conversationTitle="worker-thread"
        isChannelConversation={false}
        messagesLoading={true}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={(value) => JSON.stringify(value)}
      />
    );

    expect(container.textContent).toContain("Loading thread...");
    const threadTextarea = required(
      container.querySelector('textarea[placeholder="Reply in thread"]') as HTMLTextAreaElement | null,
      "thread reply textarea missing"
    );
    expect(threadTextarea).not.toBeNull();
    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={false}
        tasksLoading={false}
        onRefreshTasks={vi.fn()}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(41, {
            from_actor_id: "leader-agent",
            to_actor_id: null,
            route: "group_chat",
            payload: { type: "chat_message", text: "partial read state" },
          }),
        ]}
        seenByMessageId={{ 41: ["worker-agent"] }}
        humanActorId="user"
        memberLiveStates={[
          buildMemberLiveState({
            member_id: "leader-agent",
            role: "leader",
            agent_name: "Leader Agent",
          }),
          buildMemberLiveState({
            member_id: "worker-agent",
            role: "worker",
            agent_name: "Worker Agent",
          }),
          buildMemberLiveState({
            member_id: "reviewer-agent",
            role: "worker",
            agent_name: "Reviewer Agent",
          }),
        ]}
        memberIds={["leader-agent", "worker-agent", "reviewer-agent"]}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={(value) => JSON.stringify(value)}
      />
    );

    const readProgressButton = findButtonByAriaLabel(container, "Seen 1/2");
    expect(readProgressButton.getAttribute("title")).toBe("Seen 1/2");
    const progressbar = required(
      readProgressButton.querySelector('[role="progressbar"]'),
      "progressbar missing"
    );
    expect(progressbar.getAttribute("aria-valuenow")).toBe("1");
    expect(progressbar.getAttribute("aria-valuemax")).toBe("2");
  });

  it("TeamTaskPanel removes approved permission review cards from the channel after response", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const listPermissionsSpy = vi
      .spyOn(api, "listAcpPermissions")
      .mockResolvedValue([
        {
          id: "perm-1",
          agent_id: "worker-agent",
          session_id: "worker-session",
          acp_session_id: "acp-session",
          tool_call_id: "tool-call-1",
          options: [
            { option_id: "allow", name: "Allow once", kind: "allow_once" },
            { option_id: "allow_always", name: "Always allow", kind: "allow_always" },
          ],
          tool_call: { title: "git push" },
          status: "pending",
          selected_option_id: null,
          created_at: 1,
          responded_at: null,
        },
      ]);
    const respondPermissionSpy = vi.spyOn(api, "respondAcpPermission").mockResolvedValue({
      status: "ok",
    });

    try {
      renderWithMantine(
        root,
        <TeamTaskPanel
          developerMode={false}
          token="token-1"
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "permission_review_card",
                permission_id: "perm-1",
                agent_id: "worker-agent",
                tool_name: "git push",
                summary: "worker requests permission to execute git push.",
                status: "pending",
                options: [
                  { option_id: "allow", name: "Allow once", kind: "allow_once" },
                  { option_id: "allow_always", name: "Always allow", kind: "allow_always" },
                ],
              },
            }),
          ]}
          seenByMessageId={{}}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "WorkerAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "waiting for review",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );

      await waitForCondition(() => listPermissionsSpy.mock.calls.length > 0);
      await waitForCondition(() => container.textContent?.includes("Awaiting human review") ?? false);
      expect(container.textContent).toContain("git push");
      expect(container.textContent).toContain("Awaiting human review");
      expect(queryButtonByText(container, "Allow once")).not.toBeNull();
      clickElement(queryButtonByText(container, "Allow once"));

      await waitForCondition(() => respondPermissionSpy.mock.calls.length > 0);
      expect(respondPermissionSpy).toHaveBeenCalledWith("token-1", "worker-agent", "perm-1", {
        option_id: "allow",
        outcome: undefined,
      });
      await waitForCondition(
        () => container.querySelector("[data-team-permission-card='true']") === null
      );
      expect(container.textContent).not.toContain("Approved · Allow once");
      expect(queryButtonByText(container, "Allow once")).toBeNull();
      expect(queryButtonByText(container, "Cancel")).toBeNull();
      expect(container.textContent).not.toContain("worker requests permission to execute git push.");
      expect(container.textContent).not.toContain("git push");
    } finally {
      listPermissionsSpy.mockRestore();
      respondPermissionSpy.mockRestore();
    }
  });

  it("TeamTaskPanel hides timed out permission review cards even before permission polling catches up", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const listPermissionsSpy = vi.spyOn(api, "listAcpPermissions").mockResolvedValue([]);

    try {
      renderWithMantine(
        root,
        <TeamTaskPanel
          developerMode={false}
          token="token-1"
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "permission_review_card",
                permission_id: "perm-timeout-1",
                agent_id: "worker-agent",
                tool_name: "git push",
                summary: "worker requests permission to execute git push.",
                reason: "review_timeout",
                reason_text: "Agent review timed out",
                status: "pending",
                options: [
                  { option_id: "allow", name: "Allow once", kind: "allow_once" },
                  { option_id: "allow_always", name: "Always allow", kind: "allow_always" },
                ],
              },
            }),
          ]}
          seenByMessageId={{}}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "WorkerAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "waiting for review",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );

      await act(async () => {
        await Promise.resolve();
      });
      expect(listPermissionsSpy).not.toHaveBeenCalled();
      expect(container.querySelector("[data-team-permission-card='true']")).toBeNull();
      expect(container.textContent).not.toContain("git push");
      expect(container.textContent).not.toContain("Timed out");
      expect(container.textContent).not.toContain("worker requests permission to execute git push.");
      expect(container.textContent).not.toContain("Agent review timed out");
    } finally {
      listPermissionsSpy.mockRestore();
    }
  });

  it("TeamTaskPanel filters malformed permission review options before rendering actions", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const listPermissionsSpy = vi.spyOn(api, "listAcpPermissions").mockResolvedValue([
      {
        id: "perm-2",
        agent_id: "worker-agent",
        session_id: "worker-session",
        acp_session_id: "acp-session",
        tool_call_id: "tool-call-2",
        options: [
          { option_id: "allow", name: "Allow once", kind: "allow_once" },
          null,
          { option_id: "   ", name: "Blank id", kind: "deny" },
          { option_id: 7, name: "Broken id", kind: "allow_once" },
          { option_id: "allow_always", name: 99, kind: "allow_always" },
        ] as unknown as {
          option_id: string;
          name: string;
          kind: string;
        }[],
        tool_call: { title: "git push" },
        status: "pending",
        selected_option_id: null,
        created_at: 1,
        responded_at: null,
      },
    ]);

    try {
      renderWithMantine(
        root,
        <TeamTaskPanel
          developerMode={false}
          token="token-1"
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "permission_review_card",
                permission_id: "perm-2",
                agent_id: "worker-agent",
                tool_name: "git push",
                summary: "worker requests permission to execute git push.",
                status: "pending",
                options: [
                  { option_id: "allow", name: "Allow once", kind: "allow_once" },
                  null,
                  { option_id: "   ", name: "Blank id", kind: "deny" },
                  { option_id: 7, name: "Broken id", kind: "allow_once" },
                  { option_id: "allow_always", name: 99, kind: "allow_always" },
                ],
              },
            }),
          ]}
          seenByMessageId={{}}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "WorkerAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "waiting for review",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );

      await waitForCondition(() => listPermissionsSpy.mock.calls.length > 0);
      const permissionCard = required(
        container.querySelector("[data-team-permission-card='true']"),
        "permission card missing"
      ) as HTMLElement;
      expect(queryButtonByText(permissionCard, "Allow once")).not.toBeNull();
      expect(queryButtonByText(permissionCard, "Broken id")).toBeNull();
      expect(queryButtonByText(permissionCard, "99")).toBeNull();
      expect(queryButtonByText(permissionCard, "Cancel")).not.toBeNull();
      expect(
        Array.from(permissionCard.querySelectorAll("button")).filter(
          (button) => button.textContent?.trim() === "Allow once"
        )
      ).toHaveLength(1);
    } finally {
      listPermissionsSpy.mockRestore();
    }
  });

  it("TeamTaskPanel plays a tone only when a new human permission review card arrives", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const audioWindow = window as typeof window & {
      AudioContext?: unknown;
      webkitAudioContext?: unknown;
    };
    const previousAudioContext = audioWindow.AudioContext;
    const previousWebkitAudioContext = audioWindow.webkitAudioContext;
    const oscillatorStart = vi.fn();
    const oscillatorStop = vi.fn();
    const oscillatorConnect = vi.fn();
    const frequencySet = vi.fn();
    const frequencyRamp = vi.fn();
    const gainConnect = vi.fn();
    const gainSet = vi.fn();
    const gainRamp = vi.fn();

    const audioContextSpy = vi.fn();

    class MockAudioContext {
      currentTime = 1;
      destination = {};
      state = "running";

      constructor() {
        audioContextSpy();
      }

      createOscillator() {
        return {
          type: "sine",
          frequency: {
            setValueAtTime: frequencySet,
            linearRampToValueAtTime: frequencyRamp,
          },
          connect: oscillatorConnect,
          start: oscillatorStart,
          stop: oscillatorStop,
          onended: null,
        };
      }

      createGain() {
        return {
          gain: {
            setValueAtTime: gainSet,
            exponentialRampToValueAtTime: gainRamp,
          },
          connect: gainConnect,
        };
      }

      close() {
        return Promise.resolve();
      }
    }

    audioWindow.AudioContext = MockAudioContext;
    delete audioWindow.webkitAudioContext;

    const renderPanel = (messages: TeamConversationMessageRecord[]) => {
      renderWithMantine(
        root,
        <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={messages}
          seenByMessageId={{}}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "WorkerAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "waiting for review",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );
    };

    const humanReviewCardMessage = buildTaskMessage(1, {
      from_actor_id: "worker-agent",
      to_actor_id: null,
      route: "group_chat",
      payload: {
        type: "permission_review_card",
        permission_id: "perm-tone-1",
        agent_id: "worker-agent",
        tool_name: "git push",
        summary: "worker requests permission to execute git push.",
        status: "pending",
        options: [{ option_id: "allow", name: "Allow once", kind: "allow_once" }],
      },
    });

    try {
      renderPanel([]);
      await act(async () => {
        await Promise.resolve();
      });
      expect(audioContextSpy).not.toHaveBeenCalled();

      renderPanel([humanReviewCardMessage]);
      await waitForCondition(() => audioContextSpy.mock.calls.length === 1);
      expect(oscillatorStart).toHaveBeenCalledTimes(1);
      expect(oscillatorStop).toHaveBeenCalledTimes(1);
      expect(oscillatorConnect).toHaveBeenCalledTimes(1);
      expect(gainConnect).toHaveBeenCalledTimes(1);
      expect(frequencySet).toHaveBeenCalledTimes(1);
      expect(frequencyRamp).toHaveBeenCalledTimes(1);
      expect(gainSet).toHaveBeenCalledTimes(1);
      expect(gainRamp).toHaveBeenCalledTimes(2);

      renderPanel([humanReviewCardMessage]);
      await act(async () => {
        await Promise.resolve();
      });
      expect(audioContextSpy).toHaveBeenCalledTimes(1);
    } finally {
      if (previousAudioContext === undefined) {
        delete audioWindow.AudioContext;
      } else {
        audioWindow.AudioContext = previousAudioContext;
      }
      if (previousWebkitAudioContext === undefined) {
        delete audioWindow.webkitAudioContext;
      } else {
        audioWindow.webkitAudioContext = previousWebkitAudioContext;
      }
    }
  });

  it("TeamTaskPanel hides approved permission review cards after refresh even with malformed options", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));
    const listPermissionsSpy = vi.spyOn(api, "listAcpPermissions").mockResolvedValue([
      {
        id: "perm-3",
        agent_id: "worker-agent",
        session_id: "worker-session",
        acp_session_id: "acp-session",
        tool_call_id: "tool-call-3",
        options: [
          { option_id: 7, name: "Broken id", kind: "allow_once" },
          { option_id: "allow", name: "Allow once", kind: "allow_once" },
        ] as unknown as {
          option_id: string;
          name: string;
          kind: string;
        }[],
        tool_call: { title: "git push" },
        status: "responded",
        selected_option_id: "allow",
        created_at: 1,
        responded_at: 2,
      },
    ]);

    try {
      renderWithMantine(
        root,
        <TeamTaskPanel
          developerMode={false}
          token="token-1"
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "permission_review_card",
                permission_id: "perm-3",
                agent_id: "worker-agent",
                tool_name: "git push",
                summary: "worker requests permission to execute git push.",
                status: "pending",
                options: [{ option_id: "allow", name: "Allow once", kind: "allow_once" }],
              },
            }),
          ]}
          seenByMessageId={{}}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "worker-agent",
              role: "worker",
              agent_name: "WorkerAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "working",
              pending_inbox_count: 0,
              current_work: "waiting for review",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
      );

      await waitForCondition(() => listPermissionsSpy.mock.calls.length > 0);
      await waitForCondition(
        () => container.querySelector("[data-team-permission-card='true']") === null
      );
      expect(container.textContent).not.toContain("Approved · Allow once");
      expect(queryButtonByText(container, "Allow once")).toBeNull();
      expect(queryButtonByText(container, "Cancel")).toBeNull();
    } finally {
      listPermissionsSpy.mockRestore();
    }
  });

  it("TeamTaskPanel excludes the author from seen-progress counts", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "progress update" },
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
              current_work: "broadcasting update",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
    );

    const progressbar = container.querySelector('[role="progressbar"]');
    expect(progressbar?.getAttribute("aria-valuenow")).toBe("1");
    expect(progressbar?.getAttribute("aria-valuemax")).toBe("1");
  });

  it("TeamTaskPanel renders expandable activity details in developer mode", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={true}
        tasksLoading={false}
        onRefreshTasks={vi.fn()}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(7, {
            from_actor_id: "leader-agent",
            to_actor_id: "worker-agent",
            route: "to_worker",
            payload: { type: "chat_message", text: "please verify the runtime output" },
          }),
        ]}
        seenByMessageId={{ 7: ["worker-agent"] }}
        humanActorId="user"
        memberLiveStates={[
          {
            member_id: "leader-agent",
            role: "leader",
            agent_name: "LeaderAgent",
            lifecycle_status: "working",
            lifecycle_tone: "active",
            run_status: "working",
            step_status: "in_review",
            pending_inbox_count: 0,
            current_work: "triaging worker evidence",
          },
        ]}
        memberIds={["leader-agent", "worker-agent"]}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={toPrettyJson}
      />
    );

    clickElement(findButtonByText(container, "Details"));

    expect(container.textContent).toContain("source");
    expect(container.textContent).toContain("conversation");
    expect(container.textContent).toContain("seq");
    expect(container.textContent).toContain("7");
    expect(container.textContent).toContain("from");
    expect(container.textContent).toContain("leader-agent");
    expect(container.textContent).toContain("to");
    expect(container.textContent).toContain("worker-agent");
    expect(container.textContent).toContain("route");
    expect(container.textContent).toContain("to_worker");
    expect(container.textContent).toContain("work");
    expect(container.textContent).toContain("working/in_review");
    expect(container.textContent).toContain("agent");
    expect(container.textContent).toContain("current_work");
    expect(container.textContent).toContain("triaging worker evidence");
    expect(findButtonByText(container, "Hide")).toBeDefined();
    expect(container.innerHTML).not.toContain("sm:col-span-2");
  });

  it("TeamTaskPanel lets message meta controls wrap instead of forcing a single cramped row", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={true}
        tasksLoading={false}
        onRefreshTasks={vi.fn()}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(8, {
            payload: { type: "chat_message", text: "follow up with the team" },
          }),
        ]}
        seenByMessageId={{}}
        humanActorId="user:u-1"
        memberLiveStates={[buildMemberLiveState()]}
        memberIds={["leader-agent", "worker-agent"]}
        conversationTitle="Shared thread"
        isChannelConversation={true}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={(value) => JSON.stringify(value)}
        onOpenThread={vi.fn()}
        activeThreadMessageId={null}
      />
    );

    const detailsButton = findButtonByText(container, "Details");
    const threadButton = findButtonByText(container, "Thread");
    expect(detailsButton.parentElement?.className).toContain("flex-wrap");
    expect(threadButton.parentElement?.className).toContain("flex-wrap");
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
      const totalMessages = 213;
      renderWithMantine(
        root,
        <TeamTaskPanel
            developerMode={false}
            tasksLoading={false}
            onRefreshTasks={vi.fn()}
            messageDraft=""
            onMessageDraftChange={vi.fn()}
            onSendMessage={vi.fn()}
            messages={Array.from({ length: totalMessages - 1 }, (_, index) =>
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

      expect(queryButtonByAriaLabel(container, "Jump to top")).toBeNull();

      renderWithMantine(
        root,
        <TeamTaskPanel
            developerMode={false}
            tasksLoading={false}
            onRefreshTasks={vi.fn()}
            messageDraft=""
            onMessageDraftChange={vi.fn()}
            onSendMessage={vi.fn()}
            messages={Array.from({ length: totalMessages }, (_, index) =>
              buildTaskMessage(index + 1, {
                from_actor_id:
                  index === 0
                    ? "user:u-1"
                    : index === totalMessages - 1
                      ? "worker-agent"
                      : "leader-agent",
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

      await act(async () => {
        await Promise.resolve();
      });

      expect(scrollNode.scrollTop).toBe(640);
      expect(container.querySelectorAll("[data-team-channel-item='true']")).toHaveLength(10);
      expect(
        container.querySelector("[data-team-channel-top-spacer='true']")
      ).not.toBeNull();
      expect(queryButtonByAriaLabel(container, "Jump to bottom")).toBeNull();
      expect(queryButtonByAriaLabel(container, "Jump to top")).toBeNull();

      act(() => {
        scrollNode.scrollTop = 720;
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });

      expect(queryButtonByAriaLabel(container, "Jump to bottom")).toBeNull();
      expect(queryButtonByAriaLabel(container, "Jump to top")).toBeNull();

      act(() => {
        scrollNode.scrollTop = 80;
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });

      const jumpButton = queryButtonByAriaLabel(container, "Jump to bottom");
      expect(jumpButton).not.toBeNull();
      expect(queryButtonByAriaLabel(container, "Jump to top")).toBeNull();

      clickElement(jumpButton);
      expect(scrollNode.scrollTop).toBe(640);
      expect(container.querySelectorAll("[data-team-channel-item='true']")).toHaveLength(10);
      expect(queryButtonByAriaLabel(container, "Jump to bottom")).toBeNull();
      expect(queryButtonByAriaLabel(container, "Jump to top")).toBeNull();
    } finally {
      rafSpy.mockRestore();
      cancelSpy.mockRestore();
    }
  }, 20_000);

  it("TeamTaskPanel only renders markdown for the visible tail window until history is expanded", async () => {
    const markdownSpy = vi.spyOn(mailboxHelpers, "renderMarkdownWithMentions");
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    try {
      renderWithMantine(
        root,
        <TeamTaskPanel
            developerMode={false}
            tasksLoading={false}
            onRefreshTasks={vi.fn()}
            messageDraft=""
            onMessageDraftChange={vi.fn()}
            onSendMessage={vi.fn()}
            messages={Array.from({ length: 213 }, (_, index) =>
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

      await act(async () => {
        await Promise.resolve();
      });

      const initialTexts = markdownSpy.mock.calls.map((call) => call[0]);
      expect(initialTexts).not.toContain("message 1");
      expect(initialTexts).toContain("message 204");
      expect(initialTexts).toContain("message 213");

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
        value: 640,
      });
      act(() => {
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });
      act(() => {
        scrollNode.scrollTop = 0;
        scrollNode.dispatchEvent(new Event("scroll", { bubbles: true }));
      });

      const expandedTexts = markdownSpy.mock.calls.map((call) => call[0]);
      expect(expandedTexts).toContain("message 1");
    } finally {
      markdownSpy.mockRestore();
    }
  });

  it("TeamTaskPanel renders chat items inside explicit conversation bubbles", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "user:u-1",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "Need status update." },
            }),
            buildTaskMessage(2, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "Working on it." },
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

    await act(async () => {
      await Promise.resolve();
    });

    expect(container.querySelector('[data-team-channel-bubble="human"]')).not.toBeNull();
    expect(container.querySelector('[data-team-channel-bubble="agent"]')).not.toBeNull();
    expect(
      container.querySelector('[data-team-channel-bubble="agent"]')?.className
    ).toContain("rounded-[16px]");
  });

  it("TeamTaskPanel constrains rich chat bubbles for mobile-width markdown content", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "chat_message",
                text: [
                  "Long planner note with averyveryveryveryveryveryveryverylongtoken",
                  "",
                  "```sql",
                  "select * from some_really_long_table_name where planner_warning_code = 'averyveryveryveryveryveryveryverylongtoken';",
                  "```",
                ].join("\n"),
              },
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

    await act(async () => {
      await Promise.resolve();
    });

    const agentBubble = required(
      container.querySelector('[data-team-channel-bubble="agent"]') as HTMLDivElement | null,
      "agent bubble missing"
    );
    const richText = required(
      agentBubble.querySelector(".acp-text") as HTMLDivElement | null,
      "thread rich text missing"
    );

    expect(agentBubble.className).toContain("overflow-hidden");
    expect(richText.className).toContain("max-w-full");
    expect(richText.className).toContain("[overflow-wrap:anywhere]");
    expect(richText.className).toContain("[&_pre]:whitespace-pre-wrap");
    expect(richText.className).toContain("[&_pre_code]:break-words");
  });

  it("TeamTaskPanel wraps command-style bubbles without exceeding the mobile bubble width", async () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "chat_message",
                text: "$ agenthub actor team-task-note --shared-thread --text averyveryveryveryveryveryveryverylongtoken",
              },
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

    await act(async () => {
      await Promise.resolve();
    });

    const commandBody = required(
      container.querySelector('[data-team-channel-bubble="agent"] pre') as HTMLPreElement | null,
      "command-style bubble missing"
    );

    expect(commandBody.className).toContain("whitespace-pre-wrap");
    expect(commandBody.className).toContain("break-words");
  });

  it("TeamTaskPanel keeps rendered channel messages visible during background refresh", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(1, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: { type: "chat_message", text: "Existing shared-thread message" },
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["leader-agent"]}
          messagesLoading={true}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
    );

    expect(container.textContent).toContain("Existing shared-thread message");
    expect(container.textContent).not.toContain("Loading thread...");
  });

  it("TeamTaskPanel renders canonical stringified chat payloads as thread text", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={true}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
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

    expect(container.textContent).toContain("rendered from string payload");
    expect(container.textContent).not.toContain('{"type":"chat_message"');
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamTaskPanel hides task_note payloads from the channel stream", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(12, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "task_note",
                kind: "comment",
                text: "@leader idle update:\n\n- mailbox pending_count: 0\n\nAwaiting next task.",
              },
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
    );

    expect(container.textContent).toContain("No messages yet.");
    expect(container.textContent).not.toContain("idle update:");
    expect(container.textContent).not.toContain("Awaiting next task.");
    expect(container.textContent).not.toContain('"type": "task_note"');
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamTaskPanel hides non-chat ACP payloads instead of dumping raw JSON into the channel", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(13, {
              from_actor_id: "worker-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "status_update",
                stream: "acp",
                current_work: "running compile preview",
              },
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={toPrettyJson}
        />
    );

    expect(container.textContent).toContain("No messages yet.");
    expect(container.textContent).not.toContain("status_update");
    expect(container.textContent).not.toContain("running compile preview");
    expect(toPrettyJson).not.toHaveBeenCalled();
  });

  it("TeamTaskPanel renders markdown lists, tables, and code blocks in channel messages", async () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(12, {
              from_actor_id: "leader-agent",
              to_actor_id: null,
              route: "group_chat",
              payload: {
                type: "chat_message",
                text: [
                  "- item a",
                  "- item b",
                  "",
                  "| col | value |",
                  "| --- | --- |",
                  "| key | v1 |",
                  "",
                  "Inline `code` sample.",
                  "",
                  "```ts",
                  "const n = 1;",
                  "```",
                ].join("\n"),
              },
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["leader-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
    );

    await waitForCondition(() => container.innerHTML.includes("<table"));
    expect(container.innerHTML).toContain('class="md-list md-list-unordered"');
    expect(container.innerHTML).toContain('class="md-table-wrap"');
    expect(container.innerHTML).toContain("<table");
    expect(container.innerHTML).toContain("<pre");
    expect(container.innerHTML).toContain('class="md-inline-code"');
    expect(container.innerHTML).toContain(">code</code>");
  });

  it("TeamTaskPanel hides message details when developer mode is off", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[buildTaskMessage(1)]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
    );

    expect(container.textContent).not.toContain("Details");
    expect(container.textContent).not.toContain("source");
    expect(container.textContent).not.toContain("route");
  });

  it("TeamTaskPanel renders message details with shared key/value metadata when developer mode expands an item", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[
            buildTaskMessage(7, {
              from_actor_id: "leader-agent",
              to_actor_id: "worker-agent",
              route: "direct",
              payload: { type: "chat_message", text: "debug details" },
            }),
          ]}
          humanActorId="user"
          memberLiveStates={[
            {
              member_id: "leader-agent",
              role: "leader",
              agent_name: "LeaderAgent",
              lifecycle_status: "working",
              lifecycle_tone: "active",
              run_status: "working",
              step_status: "waiting",
              pending_inbox_count: 0,
              current_work: "reviewing worker progress",
            },
          ]}
          memberIds={["leader-agent", "worker-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
    );

    clickElement(findButtonByText(container, "Details"));
    expect(container.textContent).toContain("source");
    expect(container.textContent).toContain("from");
    expect(container.textContent).toContain("leader-agent");
    expect(container.textContent).toContain("to");
    expect(container.textContent).toContain("worker-agent");
    expect(container.textContent).toContain("route");
    expect(container.textContent).toContain("work");
    expect(container.textContent).toContain("working/waiting");
    expect(container.textContent).toContain("current_work");
    expect(container.textContent).toContain("reviewing worker progress");
    expect(container.innerHTML).toContain("<dl");
    expect(container.innerHTML).toContain("<dt");
    expect(container.innerHTML).toContain("<dd");
  });

  it("TeamTaskPanel renders the shared empty state when the channel has no messages", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
          developerMode={false}
          tasksLoading={false}
          onRefreshTasks={vi.fn()}
          messageDraft=""
          onMessageDraftChange={vi.fn()}
          onSendMessage={vi.fn()}
          messages={[]}
          humanActorId="user"
          memberLiveStates={[]}
          memberIds={["leader-agent"]}
          messagesLoading={false}
          busy={null}
          formatTs={(ts) => `ts-${String(ts)}`}
          toPrettyJson={(value) => JSON.stringify(value)}
        />
    );

    expect(container.textContent).toContain("No messages yet.");
  });

  it("TeamTasksPanel supports task filters, workflow guidance, linked runs, and debug compile actions", async () => {
    const onSelectedTaskIdChange = vi.fn();
    const onOpenConversation = vi.fn();
    const onCompilePreviewContextIdChange = vi.fn();
    const onCompileTaskRunPreview = vi.fn();
    const onUseCompiledRunPayload = vi.fn();
    const onCreateRunFromCompiledPreview = vi.fn();
    const onOpenRun = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            channelLabel="# review"
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
                assigned_member_id: "worker-1",
                context: { owner: "leader" },
                created_at: 90,
                updated_at: 220,
              }),
              buildPanelTask("task-3", {
                title: "Wait for PR review",
                status: "waiting",
                created_at: 85,
                updated_at: 224,
              }),
              buildPanelTask("task-4", {
                title: "Review release notes",
                status: "in_review",
                created_at: 80,
                updated_at: 225,
              }),
            ]}
            tasksLoading={false}
            selectedTaskId="task-2"
            onSelectedTaskIdChange={onSelectedTaskIdChange}
            onRefreshTasks={vi.fn()}
            onOpenConversation={onOpenConversation}
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
            memberLiveStates={[
              buildMemberLiveState(),
              buildMemberLiveState({
                member_id: "worker-1",
                role: "worker",
                agent_name: "Worker One",
              }),
            ]}
          />
        </MantineProvider>
      );
    });

    expect(container.querySelector('[data-team-surface="kanban"]')).not.toBeNull();
    expect(container.textContent).toContain("Wait for PR review");
    clickElement(findButtonByText(container, "Investigate bug"));
    await openTaskDetailModal(container, "Prepare rollout");
    clickElement(findInteractiveByText(container, "In progress", "button, label"));
    clickElement(findButtonByText(document.body, "Open thread"));
    clickElement(findButtonByText(container, "Open # review"));
    clickElement(findInteractiveByText(document.body, "Developer tools", "summary"));
    changeInputValue(
      required(
        document.body.querySelector(
          'input[placeholder="context_id override (optional)"]'
        ) as HTMLInputElement | null,
        "context input missing"
      ),
      "ctx-next"
    );
    clickElement(findButtonByText(document.body, "Compile Preview"));
    clickElement(findButtonByText(document.body, "Use Payload in Create Run"));
    clickElement(findButtonByText(document.body, "Create Run from Preview"));
    clickElement(findButtonByText(document.body, "Open Execution Run"));

    expect(document.body.querySelector('[data-team-compile-preview="true"]')).not.toBeNull();
    expect(onSelectedTaskIdChange).toHaveBeenCalledWith("task-1");
    expect(onOpenConversation).toHaveBeenNthCalledWith(1, "task-2");
    expect(onOpenConversation).toHaveBeenNthCalledWith(2);
    expect(onCompilePreviewContextIdChange).toHaveBeenCalledWith("ctx-next");
    expect(onCompileTaskRunPreview).toHaveBeenCalledTimes(1);
    expect(onUseCompiledRunPayload).toHaveBeenCalledTimes(1);
    expect(onCreateRunFromCompiledPreview).toHaveBeenCalledTimes(1);
    expect(onOpenRun).toHaveBeenCalledWith("run-2");
    expect(container.textContent).toContain("Kanban");
    expect(container.textContent).toContain("Board lanes");
    expect(container.textContent).toContain("Waiting");
    expect(container.textContent).toContain("In review");
    expect(container.textContent).toContain("Completed");
    expect(container.textContent).toContain("1");
    expect(container.textContent).toContain("Prepare rollout");
    expect(container.textContent).toContain("owner Worker One");
    expect(container.textContent).toContain(
      "Kanban is the canonical Team task surface. Human requests and clarifications should go through"
    );
    expect(container.textContent).toContain("# review");
    expect(document.body.textContent).toContain("Open thread");
    expect(container.textContent).toContain("Open # review");
    expect(document.body.textContent).toContain("Latest execution run");
    expect(document.body.textContent).toContain("Shipped the rollout summary.");
    expect(document.body.textContent).toContain("Task context");
  });

  it("TeamTasksPanel keeps details aligned with the active filter", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
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
            onOpenConversation={vi.fn()}
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
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    clickElement(findInteractiveByText(container, "Open", "button, label"));
    expect(container.textContent).toContain("Investigate bug");
    expect(container.textContent).not.toContain("Prepare rolloutAgents pick this task up automatically");
  });

  it("TeamTasksPanel keeps the kanban surface vertically scrollable", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            developerMode={false}
            tasks={[
              buildPanelTask("task-open", { title: "Investigate bug", status: "open" }),
              buildPanelTask("task-progress", {
                title: "Prepare rollout",
                status: "in_progress",
              }),
            ]}
            tasksLoading={false}
            selectedTaskId="task-open"
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
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
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    const kanbanSurface = required(
      container.querySelector('[data-team-surface="kanban"]') as HTMLDivElement | null,
      "kanban surface missing"
    );
    expect(kanbanSurface.className).toContain("overflow-y-auto");
    expect(kanbanSurface.className).toContain("overscroll-y-contain");
  });

  it("TeamTasksPanel keeps rendered task cards visible during background refresh", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            developerMode={false}
            tasks={[
              buildPanelTask("task-progress", {
                title: "Prepare rollout",
                status: "in_progress",
              }),
            ]}
            tasksLoading={true}
            selectedTaskId="task-progress"
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
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
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Prepare rollout");
    expect(container.textContent).not.toContain("Loading tasks...");
  });

  it("TeamTasksPanel renders shared empty states for an empty board", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            developerMode={false}
            tasks={[]}
            tasksLoading={false}
            selectedTaskId=""
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
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
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("No tasks yet.");
  });

  it("TeamTasksPanel covers loading, filtered no-results, and previous-runs branches", async () => {
    const onOpenRun = vi.fn();

    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            developerMode={false}
            tasks={[]}
            tasksLoading={true}
            selectedTaskId=""
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
            busy={null}
            runs={[]}
            onOpenRun={onOpenRun}
            compilePreviewContextId=""
            onCompilePreviewContextIdChange={vi.fn()}
            onCompileTaskRunPreview={vi.fn()}
            canCompileTask={false}
            compiledRunPreview={null}
            onUseCompiledRunPayload={vi.fn()}
            onCreateRunFromCompiledPreview={vi.fn()}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={(value) => JSON.stringify(value)}
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Loading tasks...");

    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
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
            onOpenConversation={vi.fn()}
            busy={null}
            runs={[
              buildRun({
                id: "run-2",
                status: "failed",
                summary: "",
                input: { task_id: "task-progress" },
                created_at: 230,
                started_at: 231,
                ended_at: 240,
              }),
              buildRun({
                id: "run-1",
                status: "failed",
                summary: "Earlier run failed.",
                input: { task_id: "task-progress" },
                created_at: 200,
                started_at: 201,
                ended_at: 210,
              }),
              buildRun({
                id: "run-0",
                status: "canceled",
                summary: "",
                input: { task_id: "task-progress" },
                created_at: 180,
                started_at: 181,
                ended_at: 182,
              }),
            ]}
            onOpenRun={onOpenRun}
            compilePreviewContextId=""
            onCompilePreviewContextIdChange={vi.fn()}
            onCompileTaskRunPreview={vi.fn()}
            canCompileTask={false}
            compiledRunPreview={null}
            onUseCompiledRunPayload={vi.fn()}
            onCreateRunFromCompiledPreview={vi.fn()}
            formatTs={(ts) => `ts-${String(ts)}`}
            toPrettyJson={(value) => JSON.stringify(value)}
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    clickElement(findInteractiveByText(container, "Canceled", "button, label"));
    expect(container.textContent).toContain("No results.");

    clickElement(findInteractiveByText(container, "In progress", "button, label"));
    await openTaskDetailModal(container, "Prepare rollout");
    expect(document.body.textContent).toContain("Latest execution run");
    expect(document.body.textContent).toContain("Latest execution run failed.");
    expect(document.body.textContent).toContain("Previous execution runs");
    expect(document.body.textContent).toContain("Earlier run failed.");
    expect(document.body.textContent).toContain("No summary recorded.");
    clickElement(findButtonByText(document.body, "run-1"));
    expect(onOpenRun).toHaveBeenCalledWith("run-1");
  });

  it("TeamTasksPanel opens task detail in a modal and closes it with the close button", async () => {
    function CompactTaskHarness() {
      const [selectedTaskId, setSelectedTaskId] = React.useState("");
      return (
        <TeamTasksPanel
          compactMode={true}
          developerMode={false}
          tasks={[
            buildPanelTask("task-open", { title: "Investigate bug", status: "open" }),
            buildPanelTask("task-progress", {
              title: "Prepare rollout",
              status: "in_progress",
            }),
          ]}
          tasksLoading={false}
          selectedTaskId={selectedTaskId}
          onSelectedTaskIdChange={setSelectedTaskId}
          onRefreshTasks={vi.fn()}
          onOpenConversation={vi.fn()}
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
          memberLiveStates={[]}
        />
      );
    }

    act(() => {
      root.render(
        <MantineProvider>
          <CompactTaskHarness />
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Board lanes");
    expect(document.body.querySelector('[role="dialog"]')).toBeNull();

    await openTaskDetailModal(container, "Prepare rollout");

    expect(container.textContent).toContain("Board lanes");
    expect(document.body.querySelector('[role="dialog"]')).not.toBeNull();
    expect(document.body.textContent).toContain("Task detail");
    expect(document.body.textContent).toContain("Latest execution run");
    expect(document.body.querySelector(".mantine-Modal-close")).not.toBeNull();

    act(() => {
      required(
        document.body.querySelector(".mantine-Modal-close") as HTMLButtonElement | null,
        "modal close button missing"
      ).click();
    });

    expect(container.textContent).toContain("Board lanes");
    await waitForCondition(() => document.body.querySelector('[role="dialog"]') === null);
  });

  it("TeamTasksPanel closes the task detail modal on Escape", async () => {
    renderWithMantine(
      root,
      <TeamTasksPanel
        compactMode={false}
        developerMode={false}
        tasks={[
          buildPanelTask("task-open", { title: "Investigate bug", status: "open" }),
          buildPanelTask("task-progress", {
            title: "Prepare rollout",
            status: "in_progress",
          }),
        ]}
        tasksLoading={false}
        selectedTaskId=""
        onSelectedTaskIdChange={vi.fn()}
        onRefreshTasks={vi.fn()}
        onOpenConversation={vi.fn()}
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
        memberLiveStates={[]}
      />
    );

    await openTaskDetailModal(container, "Prepare rollout");
    expect(document.body.querySelector('[role="dialog"]')).not.toBeNull();

    const modalDialog = required(
      document.body.querySelector('[role="dialog"]') as HTMLElement | null,
      "task detail dialog missing"
    );
    act(() => {
      modalDialog.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
      );
    });

    await waitForCondition(() => document.body.querySelector('[role="dialog"]') === null);
  });

  it("TeamTasksPanel uses terminal fallback copy for canceled latest execution runs", async () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamTasksPanel
            compactMode={false}
            developerMode={false}
            tasks={[
              buildPanelTask("task-progress", {
                title: "Prepare rollout",
                status: "in_progress",
              }),
            ]}
            tasksLoading={false}
            selectedTaskId="task-progress"
            onSelectedTaskIdChange={vi.fn()}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
            busy={null}
            runs={[
              buildRun({
                id: "run-9",
                status: "canceled",
                summary: "",
                input: { task_id: "task-progress" },
                created_at: 230,
                started_at: 231,
                ended_at: 240,
              }),
            ]}
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
            memberLiveStates={[]}
          />
        </MantineProvider>
      );
    });

    await openTaskDetailModal(container, "Prepare rollout");
    expect(document.body.textContent).toContain("Latest execution run");
    expect(document.body.textContent).toContain("Latest execution run was canceled.");
  });

  it("TeamTasksPanel covers compact reset, run warning tones, and debug disclosure toggles", async () => {
    function CompactTaskHarness() {
      const [selectedTaskId, setSelectedTaskId] = React.useState("task-progress");
      const [showSelectedTask, setShowSelectedTask] = React.useState(true);
      return (
        <div>
          <button type="button" onClick={() => setShowSelectedTask(false)}>
            Hide selected task
          </button>
          <TeamTasksPanel
            compactMode={true}
            developerMode={true}
            tasks={
              showSelectedTask
                ? [
                    buildPanelTask("task-progress", {
                      title: "Prepare rollout",
                      status: "in_progress",
                      context: { owner: "leader" },
                    }),
                  ]
                : [buildPanelTask("task-open", { title: "Investigate bug", status: "open" })]
            }
            tasksLoading={false}
            selectedTaskId={selectedTaskId}
            onSelectedTaskIdChange={setSelectedTaskId}
            onRefreshTasks={vi.fn()}
            onOpenConversation={vi.fn()}
            busy={null}
            runs={[
              buildRun({
                id: "run-input",
                status: "input_required",
                summary: "Need human input.",
                input: { task_id: "task-progress" },
              }),
              buildRun({
                id: "run-submitted",
                status: "submitted",
                summary: "Queued.",
                input: { task_id: "task-progress" },
              }),
            ]}
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
            memberLiveStates={[]}
          />
        </div>
      );
    }

    renderWithMantine(root, <CompactTaskHarness />);

    await openTaskDetailModal(container, "Prepare rollout");
    expect(document.body.textContent).toContain("Prepare rollout");
    expect(document.body.textContent).toContain("Need human input.");
    expect(document.body.innerHTML).toContain("title=\"run status: input_required\"");
    expect(document.body.innerHTML).toContain("title=\"run status: submitted\"");

    const details = required(
      document.body.querySelector("details") as HTMLDetailsElement | null,
      "developer details missing"
    );
    act(() => {
      details.open = true;
      details.dispatchEvent(new Event("toggle", { bubbles: true }));
    });
    expect(document.body.textContent).toContain("Hide");

    clickElement(findButtonByText(container, "Hide selected task"));
    expect(container.textContent).toContain("Investigate bug");
    expect(container.textContent).not.toContain("Prepare rollout");
  });

  it("TeamMemberAcpPanel renders ACP conversation for selected member", () => {
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

    renderWithMantine(
      root,
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
          onLoadOlder={onLoadOlder}
        />
    );

    expect(container.querySelector("h3")).toBeNull();
    expect(container.textContent).toContain("Activity");
    expect(container.textContent).toContain("Plan");
    expect(container.textContent).toContain("Inspect");
    expect(container.textContent).toContain("Please investigate this issue.");
    expect(container.textContent).toContain("Acknowledged. I am checking logs now.");
    expect(onLoadOlder).toHaveBeenCalledTimes(0);
  });
  it("TeamMemberAcpPanel mirrors member header status and model metadata", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        memberTitle="Worker agent"
        selectedMemberSnapshot={buildMemberSnapshot({
          member_id: "worker-agent",
          role: "worker",
          model: "gpt-5",
          status: "working",
          latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
        })}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("Worker agent");
    expect(container.textContent).toContain("gpt-5");
    expect(container.textContent).toContain("working");
    expect(container.textContent).toContain("role");
    expect(container.textContent).toContain("worker");
    expect(container.textContent).toContain("Details");
    expect(container.textContent).not.toContain("member");
    expect(container.textContent).not.toContain("session");
    expect(container.textContent).not.toContain("role=worker");
    expect(container.textContent).not.toContain("worker-agent");
  });

  it("TeamMemberAcpPanel can hide the inner member title when the outer workspace already shows it", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        memberTitle="Worker agent"
        hideMemberTitle={true}
        selectedMemberSnapshot={buildMemberSnapshot({
          member_id: "worker-agent",
          role: "worker",
          model: "gpt-5",
          status: "working",
          latest_step: buildStep({ member_id: "worker-agent", remote_task_id: "task-77" }),
        })}
        memberEvents={[]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).not.toContain("Worker agent");
    expect(container.textContent).toContain("gpt-5");
    expect(container.textContent).toContain("working");
  });

  it("TeamMemberAcpPanel keeps the ACP body in a dedicated flex shell above the input dock", () => {
    renderWithMantine(
      root,
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
            event_id: 25,
            agent_id: "worker-agent",
            session_id: "task-77",
            seq: "25",
            ts: 1_700_000_205,
            stream: "acp",
            message: JSON.stringify({
              type: "agent_message",
              text: "Panel layout should keep the ACP body above the input dock.",
            }),
          },
        ]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onSendInput={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    const acpRoot = required(
      container.querySelector(".acp") as HTMLDivElement | null,
      "team member acp root missing"
    );
    const acpShell = required(
      acpRoot.parentElement as HTMLDivElement | null,
      "team member acp shell missing"
    );
    expect(acpShell.classList.contains("min-h-0")).toBe(true);
    expect(acpShell.classList.contains("flex")).toBe(true);
    expect(acpShell.classList.contains("flex-1")).toBe(true);
    expect(acpShell.classList.contains("flex-col")).toBe(true);
    expect(acpShell.classList.contains("overflow-hidden")).toBe(true);

    const header = required(
      acpShell.previousElementSibling as HTMLDivElement | null,
      "team member header missing"
    );
    expect(header.className).toContain("output-header");
    expect(container.querySelector("textarea")).not.toBeNull();
    expect(container.textContent).not.toContain("Refresh");
    expect(container.textContent).not.toContain("Load Older");
  });

  it("TeamTaskPanel keeps the composer pinned without adding channel toolbar chrome", () => {
    renderWithMantine(
      root,
      <TeamTaskPanel
        developerMode={true}
        messageDraft=""
        onMessageDraftChange={vi.fn()}
        onSendMessage={vi.fn()}
        messages={[
          buildTaskMessage(21, {
            from_actor_id: "leader-agent",
            payload: { type: "chat_message", text: "latest message" },
          }),
        ]}
        messagesLoading={false}
        busy={null}
        formatTs={(ts) => `ts-${String(ts)}`}
        toPrettyJson={(value) => JSON.stringify(value)}
      />
    );

    const channelBody = required(
      container.querySelector('[data-team-channel-body="true"]') as HTMLDivElement | null,
      "channel body missing"
    );
    const composer = required(
      container.querySelector('[data-team-channel-composer="true"]') as HTMLDivElement | null,
      "channel composer missing"
    );

    expect(channelBody.classList.contains("flex")).toBe(true);
    expect(composer.classList.contains("shrink-0")).toBe(true);
    const bubble = required(
      container.querySelector('[data-team-channel-bubble="agent"]') as HTMLDivElement | null,
      "channel bubble missing"
    );
    expect(bubble.className).toContain("rounded-[16px]");
  });

  it("TeamMemberAcpPanel exposes a force-new-session action in debug mode", async () => {
    const onForceNewSession = vi.fn();

    renderWithMantine(
      root,
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
        onForceNewSession={onForceNewSession}
        onLoadOlder={vi.fn()}
      />
    );

    await openDebugTabAndWait(container);
    clickElement(findButtonByText(container, "Force New Session"));
    expect(onForceNewSession).toHaveBeenCalledTimes(1);
  });

  it("TeamMemberAcpPanel disables ACP debug actions when the corresponding handlers are unavailable", async () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedMemberSnapshot={null}
        selectedMemberRole="worker"
        selectedSessionId="runtime-session-1"
        memberEvents={[
          {
            event_id: 33,
            agent_id: "worker-agent",
            session_id: "runtime-session-1",
            seq: "33",
            ts: 1_700_000_333,
            stream: "acp",
            message: JSON.stringify({
              type: "config_option_update",
              config_options: [
                {
                  id: "mode",
                  label: "Mode",
                  current_value: { type: "value_id", value: "workspace_write" },
                  select_options: [
                    { value_id: "workspace_write", label: "Workspace Write" },
                  ],
                },
              ],
            }),
          },
        ]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        canControlAcp={true}
        onLoadOlder={vi.fn()}
      />
    );

    await openDebugTabAndWait(container);

    expect(findButtonByText(container, "Set Mode").disabled).toBe(true);
    expect(findButtonByText(container, "Set Model").disabled).toBe(true);
    expect(findButtonByText(container, "Set Config").disabled).toBe(true);
    expect(findButtonByText(container, "Cancel Run").disabled).toBe(true);
    expect(findButtonByText(container, "Clear Session").disabled).toBe(true);
  });

  it("TeamMemberAcpPanel disables Cancel Run when no ACP run is interruptible", async () => {
    renderWithMantine(
      root,
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
        canControlAcp={true}
        canInterrupt={true}
        onInterrupt={vi.fn()}
        onLoadOlder={vi.fn()}
      />
    );

    await openDebugTabAndWait(container);

    expect(findButtonByText(container, "Cancel Run").disabled).toBe(true);
  });

  it("TeamMemberAcpPanel auto-loads older ACP history for short threads and renders agent thinking", async () => {
    vi.useFakeTimers();
    const onLoadOlder = vi.fn();

    try {
      renderWithMantine(
        root,
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
            onLoadOlder={onLoadOlder}
          />
      );

      await act(async () => {
        vi.advanceTimersByTime(1200);
        await Promise.resolve();
      });

      expect(onLoadOlder).toHaveBeenCalled();
      expect(container.textContent).toContain(
        "Inspecting the previous failure before replying."
      );
      expect(container.textContent).toContain("I found the relevant stack trace.");
    } finally {
      vi.useRealTimers();
    }
  });

  it("TeamMemberAcpPanel shows active thinking status in the header", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_700_000_208_000);

    renderWithMantine(
      root,
      <TeamMemberAcpPanel
        developerMode={true}
        selectedMemberId="worker-agent"
        selectedMemberSnapshot={buildMemberSnapshot({
          member_id: "worker-agent",
          role: "worker",
          status: "working",
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
        ]}
        memberEventsHasMore={false}
        memberEventsLoading={false}
        eventsLoading={false}
        oldestMemberEventId={null}
        onLoadOlder={vi.fn()}
      />
    );

    expect(container.textContent).toContain("working · thinking 5s");
    nowSpy.mockRestore();
  });

  it("TeamMemberAcpPanel hides technical metadata when developer mode is off", () => {
    renderWithMantine(
      root,
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
          onLoadOlder={vi.fn()}
        />
    );

    expect(container.textContent).toContain("Activity");
    expect(container.textContent).toContain("Plan");
    expect(container.textContent).not.toContain("Inspect");
    expect(container.textContent).not.toContain("Refresh");
    expect(container.textContent).toContain("Details");
    expect(container.textContent).not.toContain("member=worker-agent");
    expect(container.textContent).not.toContain("role=worker");
    expect(container.textContent).not.toContain("session=task-77");
  });

  it("TeamMemberAcpPanel renders ACP conversation from runtime session fallback", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
          developerMode={true}
          selectedMemberId="worker-agent"
          memberTitle="Worker agent"
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
          onLoadOlder={vi.fn()}
        />
    );

    expect(container.textContent).toContain("Runtime session fallback works.");
  });

  it("TeamMemberAcpPanel keeps ACP shell visible when selected member has no session yet", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
          developerMode={false}
          selectedMemberId="worker-agent"
          memberTitle="Worker agent"
          selectedMemberSnapshot={null}
          selectedMemberRole="worker"
          memberEvents={[]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onLoadOlder={vi.fn()}
        />
    );

    expect(container.textContent).toContain("No active thread session yet");
    expect(container.textContent).toContain("Activity");
    expect(container.textContent).toContain("Plan");
  });

  it("TeamMemberAcpPanel keeps ACP shell visible when the session has no thread events yet", () => {
    renderWithMantine(
      root,
      <TeamMemberAcpPanel
          developerMode={false}
          selectedMemberId="worker-agent"
          memberTitle="Worker agent"
          selectedMemberSnapshot={null}
          selectedMemberRole="worker"
          selectedSessionId="runtime-session-1"
          memberEvents={[]}
          memberEventsHasMore={false}
          memberEventsLoading={false}
          eventsLoading={false}
          oldestMemberEventId={null}
          onLoadOlder={vi.fn()}
        />
    );

    expect(container.textContent).toContain("Active thread has no events yet");
    expect(container.textContent).toContain("Activity");
    expect(container.textContent).toContain("Plan");
  });

  it("TeamMemberAcpPanel sends prompt through ACP input dock", async () => {
    const onSendInput = vi.fn().mockResolvedValue(undefined);

    renderWithMantine(
      root,
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
          onLoadOlder={vi.fn()}
        />
    );

    const input = required(
      container.querySelector("textarea") as HTMLTextAreaElement | null,
      "ACP input textarea missing"
    );
    changeInputValue(input, "hello from team acp");
    await act(async () => {
      required(
        container.querySelector('button[aria-label="Send input"]') as HTMLButtonElement | null,
        "send input button missing"
      ).dispatchEvent(
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

    renderWithMantine(
      root,
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
        onLoadOlder={vi.fn()}
      />
    );

    const input = required(
      container.querySelector("textarea") as HTMLTextAreaElement | null,
      "ACP input textarea missing"
    );
    changeInputValue(input, "hello from team acp");
    const sendButton = required(
      container.querySelector('button[aria-label="Send input"]') as HTMLButtonElement | null,
      "send input button missing"
    );
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

  it("TeamMailboxPanel handles member chat, accept, and advanced mailbox controls", () => {
    const onSelectMember = vi.fn();
    const onConversationScroll = vi.fn();
    const onJumpToBottom = vi.fn();
    const onAcceptMessage = vi.fn();
    const onAcceptVisibleMessages = vi.fn();
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
    const pendingForLeaderMessage = buildMailboxMessage(3, {
      from_actor_id: "worker-agent",
      to_actor_id: "leader-agent",
      payload: { type: "chat_message", text: "pending-to-leader" },
    });
    const deliveredMessage = buildMailboxMessage(2, {
      from_actor_id: "worker-agent",
      to_actor_id: "leader-agent",
      status: "delivered",
      payload: { type: "status_update", done: true },
      delivered_at: 1_700_000_200,
    });

    act(() => {
      root.render(
        <MantineProvider>
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
            conversationMessages={[pendingMessage, pendingForLeaderMessage, deliveredMessage]}
            toPrettyJson={toPrettyJson}
            formatTs={(ts) => `ts-${String(ts)}`}
            busy={null}
            onAcceptMessage={onAcceptMessage}
            onAcceptVisibleMessages={onAcceptVisibleMessages}
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
        </MantineProvider>
      );
    });

    clickElement(required(container.querySelector(".teams-chat-members .team-item"), "member button missing"));

    act(() => {
      required(container.querySelector(".teams-chat-messages"), "chat list missing").dispatchEvent(
        new Event("scroll", { bubbles: true })
      );
    });

    clickElement(
      required(
        Array.from(container.querySelectorAll(".teams-chat-messages button")).find((candidate) =>
          candidate.textContent?.includes("Accept")
        ) as HTMLButtonElement | undefined,
        "message accept button missing"
      )
    );
    clickElement(findButtonByText(container, "Accept visible pending"));
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
    expect(onAcceptMessage).toHaveBeenCalledWith(pendingMessage);
    expect(onAcceptVisibleMessages).toHaveBeenCalledWith([pendingMessage]);
    expect(onChatDraftChange).toHaveBeenCalledWith("hello worker");
    expect(onSendChatMessage).toHaveBeenCalledTimes(2);
    expect(toPrettyJson).toHaveBeenCalledWith({ type: "status_update", done: true });
    expect(container.textContent).toContain("Leader Agent → Worker Agent");
    expect(container.textContent).toContain("Worker Agent (worker)");
    expect(
      required(container.querySelector(".teams-chat-head"), "mailbox header missing").textContent
    ).toContain("auto_follow=on");
    expect(container.querySelectorAll(".teams-member-unread")).toHaveLength(2);
    expect(
      required(container.querySelector(".teams-chat-panel"), "mailbox panel missing").className
    ).toContain("p-3");
    expect(
      required(
        container.querySelector(".teams-message-bubble-incoming"),
        "mailbox incoming bubble missing"
      ).className
    ).toContain("rounded-[16px]");

    act(() => {
      root.render(
        <MantineProvider>
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
            conversationMessages={[pendingMessage, pendingForLeaderMessage, deliveredMessage]}
            toPrettyJson={toPrettyJson}
            formatTs={(ts) => `ts-${String(ts)}`}
            busy={null}
            onAcceptMessage={onAcceptMessage}
            onAcceptVisibleMessages={onAcceptVisibleMessages}
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
        </MantineProvider>
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
    clickElement(findButtonByAriaLabel(container, "Refresh read-only inbox"));

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
        <MantineProvider>
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
            onAcceptMessage={() => {}}
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
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("No members available.");
    expect(container.textContent).toContain("No conversation records yet for this pair.");
  });

  it("TeamMailboxPanel disables accept actions while a mailbox accept is already in progress", () => {
    const pendingMessage = buildMailboxMessage(1);

    act(() => {
      root.render(
        <MantineProvider>
          <TeamMailboxPanel
            developerMode={true}
            snapshot={buildSnapshot()}
            displayNameByActorId={{
              "leader-agent": "Leader Agent",
              "worker-agent": "Worker Agent",
            }}
            selectedMemberId="worker-agent"
            unreadByMemberId={{ "worker-agent": 1 }}
            onSelectMember={() => {}}
            chatActors={{
              fromActorId: "leader-agent",
              toActorId: "worker-agent",
              inboxActorId: "worker-agent",
            }}
            chatStickToBottom={true}
            chatMessagesRef={React.createRef<HTMLUListElement>()}
            onConversationScroll={() => {}}
            onJumpToBottom={() => {}}
            conversationMessages={[pendingMessage]}
            toPrettyJson={(value) => JSON.stringify(value)}
            formatTs={(ts) => String(ts)}
            busy="accept-visible"
            onAcceptMessage={vi.fn()}
            onAcceptVisibleMessages={vi.fn()}
            chatDraft=""
            onChatDraftChange={() => {}}
            onSendChatMessage={() => {}}
            msgFromActorId="leader-agent"
            onMsgFromActorIdChange={() => {}}
            msgToActorId="worker-agent"
            onMsgToActorIdChange={() => {}}
            msgChannel="default"
            onMsgChannelChange={() => {}}
            msgTransport="local"
            onMsgTransportChange={() => {}}
            msgRoute="{}"
            onMsgRouteChange={() => {}}
            mailboxTemplateOptions={[]}
            msgTemplate=""
            onMsgTemplateChange={() => {}}
            onApplyMessageTemplate={() => {}}
            msgPayload="{}"
            onMsgPayloadChange={() => {}}
            msgIdempotencyKey=""
            onMsgIdempotencyKeyChange={() => {}}
            onSendMessage={() => {}}
            inboxActorId="worker-agent"
            onInboxActorIdChange={() => {}}
            inboxLimit="20"
            onInboxLimitChange={() => {}}
            inboxAfterId=""
            onInboxAfterIdChange={() => {}}
            inboxIncludeDelivered={false}
            onInboxIncludeDeliveredChange={() => {}}
            onRefreshInbox={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(findButtonByText(container, "Accept").disabled).toBe(true);
    expect(findButtonByText(container, "Accept visible pending").disabled).toBe(true);
  });

  it("TeamMailboxPanel disables refresh while any mailbox action is busy", () => {
    act(() => {
      root.render(
        <MantineProvider>
          <TeamMailboxPanel
            developerMode={true}
            mode="advanced_only"
            snapshot={buildSnapshot()}
            displayNameByActorId={{
              "leader-agent": "Leader Agent",
              "worker-agent": "Worker Agent",
            }}
            selectedMemberId="worker-agent"
            unreadByMemberId={{}}
            onSelectMember={() => {}}
            chatActors={{
              fromActorId: "leader-agent",
              toActorId: "worker-agent",
              inboxActorId: "worker-agent",
            }}
            chatStickToBottom={true}
            chatMessagesRef={React.createRef<HTMLUListElement>()}
            onConversationScroll={() => {}}
            onJumpToBottom={() => {}}
            conversationMessages={[]}
            toPrettyJson={(value) => JSON.stringify(value)}
            formatTs={(ts) => String(ts)}
            busy="send-message"
            onAcceptMessage={vi.fn()}
            onAcceptVisibleMessages={vi.fn()}
            chatDraft=""
            onChatDraftChange={() => {}}
            onSendChatMessage={() => {}}
            msgFromActorId="leader-agent"
            onMsgFromActorIdChange={() => {}}
            msgToActorId="worker-agent"
            onMsgToActorIdChange={() => {}}
            msgChannel="default"
            onMsgChannelChange={() => {}}
            msgTransport="local"
            onMsgTransportChange={() => {}}
            msgRoute="{}"
            onMsgRouteChange={() => {}}
            mailboxTemplateOptions={[]}
            msgTemplate=""
            onMsgTemplateChange={() => {}}
            onApplyMessageTemplate={() => {}}
            msgPayload="{}"
            onMsgPayloadChange={() => {}}
            msgIdempotencyKey=""
            onMsgIdempotencyKeyChange={() => {}}
            onSendMessage={() => {}}
            inboxActorId="worker-agent"
            onInboxActorIdChange={() => {}}
            inboxLimit="20"
            onInboxLimitChange={() => {}}
            inboxAfterId=""
            onInboxAfterIdChange={() => {}}
            inboxIncludeDelivered={false}
            onInboxIncludeDeliveredChange={() => {}}
            onRefreshInbox={() => {}}
          />
        </MantineProvider>
      );
    });

    expect(findButtonByAriaLabel(container, "Refresh read-only inbox").disabled).toBe(true);
  });

  it("TeamMailboxPanel renders chat_message payload strings as plain text", () => {
    const toPrettyJson = vi.fn((value: unknown) => JSON.stringify(value));

    act(() => {
      root.render(
        <MantineProvider>
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
            onAcceptMessage={vi.fn()}
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
        </MantineProvider>
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
        <MantineProvider>
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
            onAcceptMessage={vi.fn()}
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
        </MantineProvider>
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
        <MantineProvider>
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
            onAcceptMessage={vi.fn()}
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
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain("Leader Agent → You");
  });

  it("TeamMailboxPanel hides raw mailbox tools when developer mode is off", () => {
    act(() => {
      root.render(
        <MantineProvider>
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
            onAcceptMessage={vi.fn()}
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
        </MantineProvider>
      );
    });

    expect(container.textContent).toContain(
      "Enable Developer Mode in Admin to access raw mailbox tools."
    );
    expect(container.textContent).not.toContain("Advanced mailbox controls");
    expect(container.textContent).not.toContain("from_actor_id");
  });
});
