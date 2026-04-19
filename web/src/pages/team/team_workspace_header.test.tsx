import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamWorkspaceHeader } from "./team_workspace_header";
import type {
  AgentWorkspaceStatusView,
  TeamMemberAgentControlState,
  TeamRuntimeControlTone,
} from "./page_helpers";

const baseChrome = {
  mutedButtonClassName: "muted",
  headerActionButtonClassName: "header-action",
  toolbarClassName: "toolbar",
  toolbarButtonActiveClassName: "toolbar-active",
  toolbarButtonIdleClassName: "toolbar-idle",
  noticeClassName: "notice",
  noticeTextClassName: "notice-text",
  runMetaItemClassName: "run-meta",
};

const baseAgentStatusView: AgentWorkspaceStatusView = {
  role: "worker",
  lifecycle: "working",
  work: "working",
  inbox: "2",
  currentWork: "Investigating failure.",
};

const baseAgentControlState: TeamMemberAgentControlState = {
  canStart: true,
  canStop: true,
  canDelete: true,
};

const baseRuntimeTone: TeamRuntimeControlTone = {
  statusColor: "teal",
  countColor: "teal",
};

function renderHtml(override: Partial<React.ComponentProps<typeof TeamWorkspaceHeader>> = {}) {
  return renderToStaticMarkup(
    <MantineProvider>
      <TeamWorkspaceHeader
        workspaceEyebrow={null}
        showDedicatedWorkspaceHeading
        workspaceTitle="# all"
        workspaceDescription="Shared channel"
        isAgentWorkspace={false}
        selectedAgentLabel="worker-1"
        selectedAgentWorkspaceMemberId="member-1"
        selectedAgentStatusView={baseAgentStatusView}
        selectedAgentSpecDraft={null}
        selectedAgentControlState={baseAgentControlState}
        showWorkspaceRuntimeBadge
        selectedTeamRuntimeStatusLabel="team running"
        selectedTeamRuntimeOnline={3}
        selectedTeamRuntimeTotal={3}
        selectedTeamRuntimeControlTone={baseRuntimeTone}
        workspaceAdvancedTabItems={[{ value: "runs", label: "Execution Runs" }]}
        isAdvancedWorkspace={false}
        showRunActionsInAdvanced
        activeRunStatus="working"
        canResumeActiveRun
        canRestartActiveRun
        developerMode
        workspaceDetailsOpen
        workspaceDetailItems={["team=abc", "run=working"]}
        workspaceNoticeText="team running · 3 online"
        workspaceNoticeDotClassName="dot"
        workflowTabItems={[]}
        tab="conversation"
        busy={null}
        chrome={baseChrome}
        onTabChange={vi.fn()}
        onToggleWorkspaceDetails={vi.fn()}
        onRefreshActiveRun={vi.fn()}
        onCancelRun={vi.fn()}
        onResumeRun={vi.fn()}
        onRestartRun={vi.fn()}
        onOpenTeamMemberEditModal={vi.fn()}
        onStartSelectedTeamAgent={vi.fn()}
        onStopSelectedTeamAgent={vi.fn()}
        onDeleteSelectedTeamAgent={vi.fn()}
        {...override}
      />
    </MantineProvider>
  );
}

describe("TeamWorkspaceHeader", () => {
  it("renders shared workspace header with runtime badge and details", () => {
    const html = renderHtml();
    expect(html).toContain("# all");
    expect(html).toContain("Shared channel");
    expect(html).toContain("team running");
    expect(html).toContain("3/3");
    expect(html).toContain("aria-label=\"More\"");
    expect(html).toContain("team=abc");
    expect(html).toContain("max-w-[64ch]");
    expect(html).toContain("rounded-md");
  });

  it("renders agent workspace menu actions", () => {
    const html = renderHtml({
      isAgentWorkspace: true,
      workspaceTitle: "worker-1",
      workspaceDescription: null,
      showWorkspaceRuntimeBadge: false,
        showRunActionsInAdvanced: false,
        workspaceAdvancedTabItems: [],
    });
    expect(html).toContain("aria-label=\"Agent\"");
    expect(html).toContain("worker-1");
    expect(html).toContain("team=abc");
    expect(html).not.toContain("3/3 online");
  });
});
