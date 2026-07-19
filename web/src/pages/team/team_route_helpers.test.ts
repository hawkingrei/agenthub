import { describe, expect, it } from "vitest";

import {
  buildTeamChannelProfileClosePath,
  buildTeamChannelProfilePath,
  buildTeamChannelPath,
  buildTeamChannelTaskPath,
  buildTeamChannelThreadPath,
  buildTeamDetailPath,
  buildTeamLensNavigationPath,
  buildTeamMemberWorkspacePath,
  buildTeamSearchCompatibilityPath,
  buildTeamSelectorPath,
  buildTeamTaskPath,
  buildTeamTabCompatibilityPath,
  buildTeamWorkspaceLensPath,
  buildTeamWorkspacePath,
  normalizeTeamWorkspaceLensForHeader,
  resolveActiveTeamWorkspaceLens,
  resolveTeamChannelId,
  resolveTeamSelectedMemberId,
  resolveTeamSelectedTaskId,
  resolveTeamRouteSelection,
  resolveTeamSidebarSubjectPane,
  resolveTeamTabForWorkspaceLens,
  resolveTeamThreadRootMessageId,
  resolveTeamWorkspaceLens,
  resolveTeamWorkspaceTab,
  resolveWorkspaceLensForTeamTab,
  splitTeamRoutePath,
} from "./team_route_helpers";

describe("team route helpers", () => {
  it("builds team detail paths with escaped identifiers", () => {
    expect(buildTeamSelectorPath()).toBe("/workspace/teams");
    expect(buildTeamDetailPath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamDetailPath("team/1")).toBe("/workspace/teams/team%2F1");
  });

  it("keeps legacy team workspace query paths for compatibility-only state", () => {
    expect(buildTeamWorkspacePath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamWorkspacePath("team-1", "channels")).toBe("/workspace/teams/team-1");
    expect(buildTeamWorkspacePath("team-1", "channels", "review")).toBe(
      "/workspace/teams/team-1?channel=review"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "all", 42)).toBe(
      "/workspace/teams/team-1?thread=42"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "review", -1)).toBe(
      "/workspace/teams/team-1?channel=review"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "review", null, null, null, "task-9")).toBe(
      "/workspace/teams/team-1?channel=review&task=task-9"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "all", 42, null, null, "task-9")).toBe(
      "/workspace/teams/team-1?thread=42&task=task-9"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")
    ).toBe("/workspace/teams/team-1?lens=members&member=worker-1&tab=thread");
    expect(
      buildTeamWorkspacePath("team-1", "channels", "review", null, null, null, "task-9")
    ).toBe("/workspace/teams/team-1?channel=review&task=task-9");
    expect(buildTeamWorkspacePath("team-1", "members", null, null, " worker-2 ", null)).toBe(
      "/workspace/teams/team-1?lens=members&member=worker-2"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, "worker-3", "conversation" as never)
    ).toBe("/workspace/teams/team-1?lens=members&member=worker-3");
    expect(
      buildTeamWorkspacePath(
        "team-1",
        "members",
        null,
        null,
        "worker-3",
        "conversation" as never,
        "task-9"
      )
    ).toBe("/workspace/teams/team-1?lens=members&task=task-9&member=worker-3");
  });

  it("builds canonical channel-scoped profile panel paths", () => {
    expect(buildTeamChannelProfilePath("team-1", "review", "worker-1")).toBe(
      "/workspace/teams/team-1/channels/review/members/worker-1"
    );
    expect(buildTeamChannelProfilePath("team/1", "review/triage", "worker/1")).toBe(
      "/workspace/teams/team%2F1/channels/review%2Ftriage/members/worker%2F1"
    );
    expect(buildTeamChannelProfilePath("team-1", "all", "worker-1", "task-9")).toBe(
      "/workspace/teams/team-1/channels/all/tasks/task-9/members/worker-1"
    );
    expect(buildTeamChannelProfileClosePath("team-1", "review")).toBe(
      "/workspace/teams/team-1/channels/review"
    );
    expect(buildTeamChannelProfileClosePath("team-1", "all")).toBe("/workspace/teams/team-1");
    expect(buildTeamSearchCompatibilityPath("team-1")).toBe(
      "/workspace/teams/team-1?lens=search"
    );
    expect(buildTeamTabCompatibilityPath("/workspace/teams/team-1/channels/review", "runs")).toBe(
      "/workspace/teams/team-1/channels/review?tab=runs"
    );
    expect(
      buildTeamTabCompatibilityPath("/workspace/teams/team-1/members/worker-1", "agent_acp")
    ).toBe("/workspace/teams/team-1/members/worker-1?tab=thread");
  });

  it("builds canonical team lens navigation paths from the shared workspace route helper", () => {
    expect(buildTeamChannelPath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamChannelPath("team-1", "review")).toBe(
      "/workspace/teams/team-1/channels/review"
    );
    expect(buildTeamChannelThreadPath("team-1", "all", 42, "task-9")).toBe(
      "/workspace/teams/team-1/channels/all/threads/42"
    );
    expect(buildTeamChannelThreadPath("team-1", "review", 42)).toBe(
      "/workspace/teams/team-1/channels/review/threads/42"
    );
    expect(buildTeamChannelTaskPath("team-1", "review", "task-9")).toBe(
      "/workspace/teams/team-1/channels/review/tasks/task-9"
    );
    expect(buildTeamTaskPath("team-1", "task-9")).toBe(
      "/workspace/teams/team-1/tasks/task-9"
    );
    expect(buildTeamLensNavigationPath("team-1", "channels", null, "task-9")).toBe(
      "/workspace/teams/team-1/channels/all/tasks/task-9"
    );
    expect(buildTeamLensNavigationPath("team-1", "channels", "review", "task-9")).toBe(
      "/workspace/teams/team-1/channels/review/tasks/task-9"
    );
    expect(buildTeamLensNavigationPath("team-1", "members", null, "task-9")).toBe(
      "/workspace/teams/team-1?lens=members"
    );
    expect(buildTeamLensNavigationPath("team-1", "nodes", null, "task-9")).toBe(
      "/workspace/teams/team-1?lens=nodes"
    );
    expect(buildTeamWorkspaceLensPath("team-1", "search", "review")).toBe(
      "/workspace/teams/team-1/channels/review"
    );
    expect(buildTeamWorkspaceLensPath("team-1", "tasks", "review")).toBe(
      "/workspace/teams/team-1?lens=tasks"
    );
    expect(buildTeamWorkspaceLensPath("team-1", "members", "review")).toBe(
      "/workspace/teams/team-1?lens=members"
    );
  });

  it("builds canonical Team member workspace paths", () => {
    expect(buildTeamMemberWorkspacePath("team-1", "worker-1", "agent_acp")).toBe(
      "/workspace/teams/team-1/members/worker-1/thread"
    );
    expect(buildTeamMemberWorkspacePath("team-1", "worker-1", "member_console")).toBe(
      "/workspace/teams/team-1/members/worker-1/member_console"
    );
  });

  it("splits facade-built paths for TeamPage pathname and search props", () => {
    expect(splitTeamRoutePath(buildTeamTaskPath("team-1"))).toEqual({
      pathname: "/workspace/teams/team-1",
      search: "?lens=tasks",
    });
    expect(splitTeamRoutePath(buildTeamChannelPath("team-1", "review"))).toEqual({
      pathname: "/workspace/teams/team-1/channels/review",
      search: "",
    });
    expect(splitTeamRoutePath(buildTeamSearchCompatibilityPath("team-1"))).toEqual({
      pathname: "/workspace/teams/team-1",
      search: "?lens=search",
    });
  });

  it("parses team route query state before canonical path state", () => {
    expect(resolveTeamChannelId("")).toBe("all");
    expect(resolveTeamChannelId("?channel=all")).toBe("all");
    expect(resolveTeamChannelId("?channel=research")).toBe("research");
    expect(resolveTeamChannelId("?channel=%C4%B0nbox")).toBe("İnbox");
    expect(resolveTeamChannelId("", "/workspace/teams/team-1/channels/Review")).toBe("review");
    expect(resolveTeamChannelId("?channel=query", "/workspace/teams/team-1/channels/path")).toBe(
      "query"
    );
    expect(resolveTeamThreadRootMessageId("")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=17")).toBe(17);
    expect(
      resolveTeamThreadRootMessageId("", "/workspace/teams/team-1/channels/review/threads/17")
    ).toBe(17);
    expect(
      resolveTeamThreadRootMessageId(
        "?thread=23",
        "/workspace/teams/team-1/channels/review/threads/17"
      )
    ).toBe(23);
    expect(resolveTeamThreadRootMessageId("?thread=abc")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=0")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=-8")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=7.5")).toBeNull();
    expect(resolveTeamSelectedTaskId("")).toBe("");
    expect(resolveTeamSelectedTaskId("?task=task-7")).toBe("task-7");
    expect(resolveTeamSelectedTaskId("", "/workspace/teams/team-1/tasks/task-7")).toBe("task-7");
    expect(
      resolveTeamSelectedTaskId("?task=query-task", "/workspace/teams/team-1/tasks/path-task")
    ).toBe("query-task");
    expect(resolveTeamSelectedTaskId("?task=%20task-7%20")).toBe("task-7");
    expect(resolveTeamSelectedMemberId("?member=worker-1")).toBe("worker-1");
    expect(resolveTeamSelectedMemberId("", "/workspace/teams/team-1/members/worker-2")).toBe(
      "worker-2"
    );
    expect(
      resolveTeamSelectedMemberId("?member=query-worker", "/workspace/teams/team-1/members/path")
    ).toBe("query-worker");
    expect(resolveTeamWorkspaceTab("?tab=agent_acp")).toBe("agent_acp");
    expect(resolveTeamWorkspaceTab("?tab=thread")).toBe("agent_acp");
    expect(resolveTeamWorkspaceTab("?tab=mailbox")).toBe("mailbox");
    expect(resolveTeamWorkspaceTab("?tab=member_console")).toBe("member_console");
    expect(
      resolveTeamWorkspaceTab("", "/workspace/teams/team-1/members/worker-1/member_console")
    ).toBe("member_console");
    expect(
      resolveTeamWorkspaceTab(
        "?tab=mailbox",
        "/workspace/teams/team-1/members/worker-1/member_console"
      )
    ).toBe("mailbox");
    expect(resolveTeamWorkspaceTab("?tab=overview")).toBeNull();
    expect(resolveTeamSelectedMemberId("", "/workspace/teams/team-1/members/%E0%A4%A")).toBe(
      "%E0%A4%A"
    );
  });

  it("maps route workspace lenses to Team tabs without promoting search to a content tab", () => {
    expect(resolveTeamTabForWorkspaceLens("channels")).toBe("conversation");
    expect(resolveTeamTabForWorkspaceLens("tasks")).toBe("tasks");
    expect(resolveTeamTabForWorkspaceLens("members")).toBe("overview");
    expect(resolveTeamTabForWorkspaceLens("search")).toBeNull();
    expect(resolveTeamTabForWorkspaceLens("teams")).toBe("conversation");
  });

  it("resolves Team workspace lenses through the Team-owned route facade", () => {
    expect(resolveTeamWorkspaceLens("/workspace/teams/team-1", "")).toBe("channels");
    expect(resolveTeamWorkspaceLens("/workspace/teams/team-1/tasks/task-1", "")).toBe("tasks");
    expect(resolveTeamWorkspaceLens("/workspace/teams/team-1/members/worker-1", "")).toBe(
      "members"
    );
    expect(resolveTeamWorkspaceLens("/workspace/teams/team-1", "?lens=search")).toBe("channels");
  });

  it("resolves the Team route selection snapshot through the Team-owned facade", () => {
    expect(
      resolveTeamRouteSelection(
        "/workspace/teams/team-1/channels/path-channel/threads/17",
        "?channel=query-channel&thread=23&task=task-9&member=worker-1&tab=mailbox"
      )
    ).toEqual({
      workspaceLens: "channels",
      workspaceTab: "mailbox",
      channelId: "query-channel",
      threadRootMessageId: 23,
      selectedMemberId: "worker-1",
      selectedTaskId: "task-9",
    });

    expect(
      resolveTeamRouteSelection(
        "/workspace/teams/team-1/channels/review/tasks/task-9/members/worker-2",
        ""
      )
    ).toEqual({
      workspaceLens: "channels",
      workspaceTab: null,
      channelId: "review",
      threadRootMessageId: null,
      selectedMemberId: "worker-2",
      selectedTaskId: "task-9",
    });

    expect(
      resolveTeamRouteSelection(
        "/workspace/teams/team-1/channels/review/tasks/path-task/members/path-worker",
        "?member=query-worker&task=query-task"
      )
    ).toEqual({
      workspaceLens: "channels",
      workspaceTab: null,
      channelId: "review",
      threadRootMessageId: null,
      selectedMemberId: "query-worker",
      selectedTaskId: "query-task",
    });

    expect(resolveTeamRouteSelection("/workspace/teams/team-1/members/worker-2/thread", "")).toEqual({
      workspaceLens: "members",
      workspaceTab: "agent_acp",
      channelId: "all",
      threadRootMessageId: null,
      selectedMemberId: "worker-2",
      selectedTaskId: "",
    });
    expect(
      resolveTeamRouteSelection(
        "/workspace/teams/team-1/members/path-worker/member_console",
        "?member=query-worker&tab=mailbox"
      )
    ).toEqual({
      workspaceLens: "members",
      workspaceTab: "mailbox",
      channelId: "all",
      threadRootMessageId: null,
      selectedMemberId: "query-worker",
      selectedTaskId: "",
    });
  });

  it("resolves active Team workspace lenses from explicit routes before tab fallback", () => {
    expect(resolveWorkspaceLensForTeamTab("conversation")).toBe("channels");
    expect(resolveWorkspaceLensForTeamTab("tasks")).toBe("tasks");
    expect(resolveWorkspaceLensForTeamTab("overview")).toBe("members");
    expect(
      resolveActiveTeamWorkspaceLens({
        routeWorkspaceLens: "members",
        tab: "conversation",
      })
    ).toBe("members");
    expect(
      resolveActiveTeamWorkspaceLens({
        routeWorkspaceLens: null,
        tab: "overview",
      })
    ).toBe("members");
    expect(
      resolveActiveTeamWorkspaceLens({
        routeWorkspaceLens: "search",
        tab: "tasks",
      })
    ).toBe("tasks");
    expect(
      resolveActiveTeamWorkspaceLens({
        routeWorkspaceLens: "search",
        tab: "conversation",
      })
    ).toBe("channels");
  });

  it("normalizes deprecated search lens for Team workspace header decisions", () => {
    expect(normalizeTeamWorkspaceLensForHeader("search")).toBe("channels");
    expect(normalizeTeamWorkspaceLensForHeader("channels")).toBe("channels");
  });

  it("resolves Team sidebar subject panes from workspace lenses before tab fallback", () => {
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "conversation",
        activeWorkspaceLens: "tasks",
      })
    ).toBe("tasks");
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "conversation",
        activeWorkspaceLens: "members",
      })
    ).toBe("agents");
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "agent_acp",
        activeWorkspaceLens: "channels",
      })
    ).toBe("agents");
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "tasks",
        activeWorkspaceLens: "channels",
      })
    ).toBe("tasks");
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "conversation",
        activeWorkspaceLens: "search",
      })
    ).toBe("channels");
    expect(
      resolveTeamSidebarSubjectPane({
        tab: "mailbox",
        activeWorkspaceLens: null,
      })
    ).toBe("agents");
  });
});
