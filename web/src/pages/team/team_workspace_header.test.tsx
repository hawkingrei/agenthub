import { MantineProvider } from "@mantine/core";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { TeamWorkspaceHeader } from "./team_workspace_header";
import type { TeamMemberProfileDraft } from "./create_helpers";
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

const baseAgentSpecDraft: TeamMemberProfileDraft = {
  member_id: "worker-1",
  description: "Own planning and keep the team aligned.",
  prompt: "",
  skills: [],
  custom_skills: "",
  model: "",
  role: "worker" as const,
  agent_loop_enabled: false,
  agent_loop_idle_seconds: "",
  agent_loop_prompt: "",
  codex_acp_default_mode: "full-access",
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
        selectedAgentSpecDraft={baseAgentSpecDraft}
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
    expect(html).toContain("flex min-w-0 flex-nowrap items-center justify-between");
    expect(html).toContain("hidden items-center gap-1.5 rounded-md");
    expect(html).toContain("sm:inline-flex");
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
    expect(html).toContain("Own planning and keep the team aligned.");
    expect(html).toContain('data-avatar-seed="worker-1::member-1"');
    expect(html).toContain("team=abc");
    expect(html).not.toContain("3/3 online");
  });

  it("keeps workflow navigation out of the compact workspace header", () => {
    const html = renderHtml({
      workflowTabItems: [
        { value: "conversation", label: "Channels" },
        { value: "tasks", label: "Tasks" },
        { value: "runs", label: "Runs" },
      ],
    });

    expect(html).not.toContain('data-team-surface="workflow-tabs"');
    expect(html).not.toContain(">Channels<");
    expect(html).not.toContain(">Tasks<");
    expect(html).toContain('aria-label="More"');
  });

  it("renders agent identity fallback text and enabled loop summary when the profile is sparse", () => {
    const html = renderHtml({
      isAgentWorkspace: true,
      workspaceTitle: "worker-1",
      workspaceDescription: null,
      showWorkspaceRuntimeBadge: false,
      showRunActionsInAdvanced: false,
      workspaceAdvancedTabItems: [],
      workspaceNoticeText: null,
      selectedAgentSpecDraft: {
        ...baseAgentSpecDraft,
        description: "   ",
        role: "coordinator",
        agent_loop_enabled: true,
        agent_loop_idle_seconds: "15",
      },
      selectedAgentStatusView: {
        ...baseAgentStatusView,
        role: "coordinator",
        currentWork: "Reviewing the latest rollout before handoff.",
      },
    });

    expect(html).toContain("No agent identity description yet.");
    expect(html).toContain('aria-label="Agent"');
    expect(html).toContain('title="No agent identity description yet."');
    expect(html).not.toContain("team running · 3 online");
  });

  it("hides runtime chrome and workspace details when they are disabled", () => {
    const html = renderHtml({
      showDedicatedWorkspaceHeading: false,
      workspaceEyebrow: "Shared lane",
      workspaceDescription: null,
      showWorkspaceRuntimeBadge: false,
      workspaceAdvancedTabItems: [],
      showRunActionsInAdvanced: false,
      workspaceNoticeText: null,
      developerMode: false,
      workspaceDetailsOpen: false,
      workspaceDetailItems: [],
    });

    expect(html).toContain("Shared lane");
    expect(html).not.toContain('aria-label="More"');
    expect(html).not.toContain("team running");
    expect(html).not.toContain("team=abc");
    expect(html).not.toContain('class="notice"');
  });
});
