import { describe, expect, it, vi } from "vitest";
import {
  buildTeamPageModalsProps,
  buildTeamRunsPanelProps,
  buildTeamSidebarProps,
  buildTeamWorkbenchBodyProps,
  buildTeamWorkbenchContentProps,
  buildTeamWorkspaceHeaderProps,
  type TeamWorkbenchBodyProps,
} from "./team_page_route_props";

describe("team_page_route_props builders", () => {
  it("returns the same prop objects for extracted route builders", () => {
    const sidebarProps = {
      onCreateTeam: vi.fn(),
    } as unknown as Parameters<typeof buildTeamSidebarProps>[0];
    const workbenchContentProps = {
      selectedTeamId: "team-1",
    } as unknown as Parameters<typeof buildTeamWorkbenchContentProps>[0];
    const workspaceHeaderProps = {
      title: "Team One",
    } as unknown as Parameters<typeof buildTeamWorkspaceHeaderProps>[0];
    const runsPanelProps = {
      runs: [],
    } as unknown as Parameters<typeof buildTeamRunsPanelProps>[0];
    const teamPageModalsProps = {
      createTeamModalOpen: false,
    } as unknown as Parameters<typeof buildTeamPageModalsProps>[0];
    const workbenchBodyProps = {
      conversationPanel: null,
      threadPane: null,
      tasksPanel: null,
      agentAcpPanel: null,
      overviewPanelProps: null,
      eventsPanelProps: null,
      stepsPanelProps: null,
      mailboxHasActiveRun: false,
      mailboxEmptyTitle: "No active run",
      mailboxEmptyBody: "Start a run to see mailbox activity.",
      onGoToRuns: vi.fn(),
      mailboxPanelProps: null,
      memberConsolePanelProps: null,
      debugPanel: null,
    } as unknown as TeamWorkbenchBodyProps;

    expect(buildTeamSidebarProps(sidebarProps)).toBe(sidebarProps);
    expect(buildTeamWorkbenchContentProps(workbenchContentProps)).toBe(
      workbenchContentProps
    );
    expect(buildTeamWorkspaceHeaderProps(workspaceHeaderProps)).toBe(
      workspaceHeaderProps
    );
    expect(buildTeamRunsPanelProps(runsPanelProps)).toBe(runsPanelProps);
    expect(buildTeamPageModalsProps(teamPageModalsProps)).toBe(teamPageModalsProps);
    expect(buildTeamWorkbenchBodyProps(workbenchBodyProps)).toBe(workbenchBodyProps);
  });
});
