// @vitest-environment jsdom
import { renderToStaticMarkup } from "react-dom/server";
import { createRoot, type Root } from "react-dom/client";
import { act } from "react";
import { MantineProvider } from "@mantine/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TeamDefinitionRecord } from "../../api";
import {
  TeamWorkbenchContent,
  TeamPanelLoadingFallback,
  prefetchTeamSetupSurface,
  prefetchTeamWorkbenchTab,
  shouldShowTeamWorkspaceHeader,
  type TeamWorkbenchContentProps,
} from "./team_workbench_content";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function createBaseWorkbenchContentProps(): TeamWorkbenchContentProps {
  return {
    showTeamBootstrapLoading: false,
    showTeamUnavailable: false,
    onBackToSelector: vi.fn(),
    selectedTeam: {
      team_id: "team-1",
      name: "Team One",
    } as unknown as TeamDefinitionRecord,
    isAgentWorkspace: false,
    teamSectionCardClassName: "team-card",
    panelSecondaryButtonClassName: "team-button",
    teamWorkbenchWorkspaceShellClassName: "team-shell",
    workspaceHeaderProps: {
      workspaceEyebrow: null,
      showDedicatedWorkspaceHeading: true,
      workspaceTitle: "# all",
      workspaceDescription: "Shared channel",
      isAgentWorkspace: false,
      selectedAgentLabel: "worker-1",
      selectedAgentWorkspaceMemberId: "member-1",
      selectedAgentStatusView: {
        role: "worker",
        lifecycle: "idle",
        work: "idle",
        inbox: "0",
        currentWork: "Idle",
      },
      selectedAgentSpecDraft: null,
      selectedAgentControlState: {
        canStart: false,
        canStop: false,
        canDelete: false,
      },
      showWorkspaceRuntimeBadge: false,
      selectedTeamRuntimeStatusLabel: "stopped",
      selectedTeamRuntimeOnline: 0,
      selectedTeamRuntimeTotal: 0,
      selectedTeamRuntimeControlTone: {
        statusColor: "gray",
        countColor: "gray",
      } as const,
      workspaceAdvancedTabItems: [],
      isAdvancedWorkspace: false,
      showRunActionsInAdvanced: false,
      activeRunStatus: null,
      canResumeActiveRun: false,
      canRestartActiveRun: false,
      developerMode: false,
      workspaceDetailsOpen: false,
      workspaceDetailItems: [],
      workspaceNoticeText: null,
      workspaceNoticeDotClassName: "dot",
      busy: null,
      chrome: {
        mutedButtonClassName: "muted",
        headerActionButtonClassName: "header-action",
        toolbarClassName: "toolbar",
        toolbarButtonActiveClassName: "toolbar-active",
        toolbarButtonIdleClassName: "toolbar-idle",
        noticeClassName: "notice",
        noticeTextClassName: "notice-text",
        runMetaItemClassName: "run-meta",
      },
      onTabChange: vi.fn(),
      onToggleWorkspaceDetails: vi.fn(),
      onRefreshActiveRun: vi.fn(),
      onCancelRun: vi.fn(),
      onResumeRun: vi.fn(),
      onRestartRun: vi.fn(),
      onOpenTeamMemberEditModal: vi.fn(),
      onStartSelectedTeamAgent: vi.fn(),
      onStopSelectedTeamAgent: vi.fn(),
      onDeleteSelectedTeamAgent: vi.fn(),
    },
    selectedTeamHasConfiguredMembers: true,
    selectedTeamDescription: null,
    teamMemberForgeLabel: "Add Agent",
    teamMemberCopyExistingLabel: "Copy Existing Agent",
    onOpenTeamMemberForge: vi.fn(),
    onOpenTeamMemberCopyExisting: vi.fn(),
    tab: "conversation",
    runsPanelProps: {} as TeamWorkbenchContentProps["runsPanelProps"],
    showRunContextLoading: false,
    showNoActiveRunNotice: false,
    activeWorkspaceLens: "channels",
    conversationPanel: <div data-testid="conversation-panel">Conversation</div>,
    tasksPanel: <div>Tasks</div>,
    agentAcpPanel: <div>ACP</div>,
    overviewPanelProps: null,
    eventsPanelProps: null,
    stepsPanelProps: null,
    mailboxHasActiveRun: false,
    mailboxEmptyTitle: "No active run",
    mailboxEmptyBody: "Start a run to see mailbox activity.",
    onGoToRuns: vi.fn(),
    mailboxPanelProps: null,
    memberConsolePanelProps: null,
    debugPanel: <div>Debug</div>,
  };
}

describe("team_workbench_content", () => {
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

  it("renders the shared loading fallback chrome", () => {
    act(() => {
      root.render(<TeamPanelLoadingFallback />);
    });

    expect(container.textContent).toContain("Loading workspace panel...");
    expect(container.firstElementChild?.className).toContain("rounded-2xl");
  });

  it("prefetches each lazy workbench surface without throwing", async () => {
    expect(() => prefetchTeamWorkbenchTab("runs")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("overview")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("events")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("steps")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("mailbox")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("member_console")).not.toThrow();
    expect(() => prefetchTeamWorkbenchTab("conversation")).not.toThrow();
    expect(() => prefetchTeamSetupSurface()).not.toThrow();

    await act(async () => {
      await vi.dynamicImportSettled();
    });
  });

  it("keeps the workspace header visibility rule explicit for shell reuse", () => {
    expect(
      shouldShowTeamWorkspaceHeader({
        activeWorkspaceLens: "channels",
        tab: "conversation",
      })
    ).toBe(true);
    expect(
      shouldShowTeamWorkspaceHeader({
        activeWorkspaceLens: "search",
        tab: "conversation",
      })
    ).toBe(true);
    expect(
      shouldShowTeamWorkspaceHeader({
        activeWorkspaceLens: "tasks",
        tab: "conversation",
      })
    ).toBe(false);
    expect(
      shouldShowTeamWorkspaceHeader({
        activeWorkspaceLens: "channels",
        tab: "tasks",
      })
    ).toBe(false);
  });

  it("only renders the constrained thread wrapper when a thread pane is present", () => {
    const baseProps = createBaseWorkbenchContentProps();

    const withoutThread = renderToStaticMarkup(
      <TeamWorkbenchContent {...baseProps} threadPane={null} />
    );
    expect(withoutThread).not.toContain("max-h-[40vh]");
    expect(withoutThread).not.toContain("lg:grid");
    expect(withoutThread).not.toContain("lg:grid-cols-[minmax(0,1fr)_minmax(22rem,1fr)]");

    const withThread = renderToStaticMarkup(
      <TeamWorkbenchContent
        {...baseProps}
        threadPane={<aside data-testid="thread-pane">Thread</aside>}
      />
    );
    expect(withThread).toContain("data-testid=\"thread-pane\"");
    expect(withThread).toContain("max-h-[40vh]");
    expect(withThread).toContain("lg:grid-cols-[minmax(0,1fr)_minmax(22rem,1fr)]");
    expect(withThread).toContain("w-full");
    expect(withThread).toContain("lg:h-full");
    expect(withThread).toContain("lg:min-w-0");
    expect(withThread).toContain("flex-col");
    expect(withThread).toContain('data-workspace-split-pane-layout="true"');
    expect(withThread).toContain('data-secondary-open="true"');
    expect(withThread).toContain("team-channel-pane");
    expect(withThread).toContain("team-thread-dock");
    expect(withThread).not.toContain("overflow-y-auto");
  });

  it("renders tasks as a separate workspace without the chat header", () => {
    const baseProps = createBaseWorkbenchContentProps();
    const html = renderToStaticMarkup(
      <TeamWorkbenchContent
        {...baseProps}
        tab="tasks"
        activeWorkspaceLens="tasks"
        tasksPanel={<div data-testid="tasks-panel">Task board</div>}
      />
    );

    expect(html).toContain('data-testid="tasks-panel"');
    expect(html).toContain("Task board");
    expect(html).not.toContain("# all");
    expect(html).not.toContain('aria-label="More"');
  });

  it("treats the deprecated search lens as the channel surface", () => {
    const html = renderToStaticMarkup(
      <MantineProvider env="test">
        <TeamWorkbenchContent
          {...createBaseWorkbenchContentProps()}
          activeWorkspaceLens="search"
        />
      </MantineProvider>
    );

    expect(html).toContain('data-testid="conversation-panel"');
    expect(html).not.toContain('data-workspace-lens-placeholder="search"');
    expect(html).not.toContain("Search workspace");
  });

  it("renders the workspace header with a single shell chrome class", () => {
    const html = renderToStaticMarkup(
      <TeamWorkbenchContent
        {...createBaseWorkbenchContentProps()}
        teamSectionCardClassName="teams-panel-card"
        teamWorkbenchWorkspaceShellClassName="workspace-shell"
      />
    );

    const headerShellMatch = html.match(
      /<div class="([^"]*workspace-shell[^"]*)" data-workspace-section-shell="true" data-team-workspace-header-shell="true">/
    );
    expect(headerShellMatch?.[1]).toContain("workspace-shell");
    expect(headerShellMatch?.[1]).toContain("rounded-xl");
    expect(headerShellMatch?.[1]).not.toContain("teams-panel-card");
  });

  it("uses compact shared shell chrome for agent workspaces", () => {
    const html = renderToStaticMarkup(
      <TeamWorkbenchContent
        {...createBaseWorkbenchContentProps()}
        isAgentWorkspace
        teamWorkbenchWorkspaceShellClassName="workspace-shell"
      />
    );

    const headerShellMatch = html.match(
      /<div class="([^"]*workspace-shell[^"]*)" data-workspace-section-shell="true" data-team-workspace-header-shell="true">/
    );
    expect(headerShellMatch?.[1]).toContain("py-0.5");
  });

  it("uses shared content stacks for Team workspace body layout", () => {
    const html = renderToStaticMarkup(
      <TeamWorkbenchContent
        {...createBaseWorkbenchContentProps()}
        conversationPanel={<div data-testid="conversation-panel">Conversation</div>}
      />
    );

    expect(html).toContain('data-workspace-content-stack="true"');
    expect(html).toContain('data-team-workspace-body-stack="true"');
    expect(html).toContain("gap-3");
  });
});
