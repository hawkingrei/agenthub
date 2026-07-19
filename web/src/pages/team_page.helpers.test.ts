import { describe, expect, it } from "vitest";

import {
  formatTeamRuntimeActionSummary,
  parseTeamAgentInputSessionMismatch,
  resolveNextSelectedAgentWorkspaceSessionOverride,
  resolveNextSelectedAgentWorkspaceStickySession,
  resolveChannelRouteTaskId,
  resolveRouteScopedConversationTaskSelection,
  resolveSelectedAgentWorkspaceSessionId,
  resolveThreadRootMessageIdFromPayload,
  validateRunInputJson,
} from "./team_page";
import { isCurrentTeamScopedRequest } from "./team/page_helpers";
import { shouldRefreshSelectedAgentWorkspaceSession } from "./team/use_team_member_acp_session_discovery";

describe("team_page helpers", () => {
  it("parses agent session mismatch payloads and rejects malformed messages", () => {
    expect(
      parseTeamAgentInputSessionMismatch(
        "agent session mismatch: expected=session-a running=session-b"
      )
    ).toEqual({
      expected: "session-a",
      running: "session-b",
    });
    expect(parseTeamAgentInputSessionMismatch("other error")).toBeNull();
    expect(
      parseTeamAgentInputSessionMismatch("agent session mismatch: expected= running=session-b")
    ).toBeNull();
  });

  it("resolves route-scoped conversation task selection across task and channel lanes", () => {
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "",
        routeSelectedTaskId: "task-7",
        routeChannelId: "review",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-7");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "task-7",
        routeSelectedTaskId: "task-7",
        routeChannelId: "review",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-7");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "",
        routeSelectedTaskId: "",
        routeChannelId: "all",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "task-review",
        routeSelectedTaskId: "",
        routeChannelId: "all",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "",
        routeSelectedTaskId: "",
        routeChannelId: "review",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-review");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "task-review",
        routeSelectedTaskId: "",
        routeChannelId: "review",
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-review");
    expect(
      resolveRouteScopedConversationTaskSelection({
        previousTaskId: "",
        routeSelectedTaskId: "",
        routeChannelId: "review",
        selectedChannelTaskId: null,
      })
    ).toBeNull();
  });

  it("keeps only explicit task conversations in channel-thread route state", () => {
    expect(
      resolveChannelRouteTaskId({
        routeSelectedTaskId: "task-review",
        selectedConversationTaskId: "task-review",
        selectedConversationIsShared: false,
        selectedConversationMatchesChannelLane: true,
        selectedChannelTaskId: "task-review",
      })
    ).toBeNull();
    expect(
      resolveChannelRouteTaskId({
        routeSelectedTaskId: "task-work",
        selectedConversationTaskId: "task-work",
        selectedConversationIsShared: false,
        selectedConversationMatchesChannelLane: true,
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-work");
    expect(
      resolveChannelRouteTaskId({
        routeSelectedTaskId: "",
        selectedConversationTaskId: "task-work",
        selectedConversationIsShared: false,
        selectedConversationMatchesChannelLane: true,
        selectedChannelTaskId: "task-review",
      })
    ).toBe("task-work");
    expect(
      resolveChannelRouteTaskId({
        routeSelectedTaskId: "task-review",
        selectedConversationTaskId: "",
        selectedConversationIsShared: true,
        selectedConversationMatchesChannelLane: true,
        selectedChannelTaskId: "task-review",
      })
    ).toBeNull();
    expect(
      resolveChannelRouteTaskId({
        routeSelectedTaskId: "task-review",
        selectedConversationTaskId: "task-review",
        selectedConversationIsShared: false,
        selectedConversationMatchesChannelLane: false,
        selectedChannelTaskId: "task-review",
      })
    ).toBeNull();
  });

  it("prefers runtime session ids over snapshot handles for member ACP routing", () => {
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "runtime-session",
        null,
        "running"
      )
    ).toBe("runtime-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "runtime-session",
        "sticky-session",
        "stopped",
        null,
        "running"
      )
    ).toBe("runtime-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "runtime-session",
        "sticky-session",
        "stopped",
        null,
        "stopped"
      )
    ).toBeNull();
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "   ",
        null,
        null
      )
    ).toBe("snapshot-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "new-snapshot-session",
        } as never,
        null,
        null,
        "   ",
        "sticky-session",
        "running"
      )
    ).toBe("sticky-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "runtime-session",
        "sticky-session",
        "stopped",
        "running",
        "running"
      )
    ).toBe("runtime-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        null,
        null,
        "runtime-session",
        "sticky-session",
        "stopped",
        "stopped",
        "running"
      )
    ).toBeNull();
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        "snapshot-api-session",
        "waiting_permission",
        null,
        null,
        "running"
      )
    ).toBe("snapshot-api-session");
    expect(
      resolveSelectedAgentWorkspaceSessionId(
        {
          member_id: "worker-1",
          remote_task_id: "snapshot-session",
        } as never,
        "snapshot-api-session",
        "stopped",
        null,
        null,
        "running"
      )
    ).toBeNull();
    expect(
      resolveSelectedAgentWorkspaceSessionId(null, null, null, null, null, "stopped")
    ).toBeNull();
    expect(resolveSelectedAgentWorkspaceSessionId(null, null, null, null)).toBeNull();
  });

  it("updates sticky ACP session state declaratively across member changes", () => {
    const initial = { memberId: "", sessionId: null as string | null };
    const workerOne = resolveNextSelectedAgentWorkspaceStickySession(
      initial,
      "worker-1",
      "session-1"
    );
    expect(workerOne).toEqual({ memberId: "worker-1", sessionId: "session-1" });

    expect(
      resolveNextSelectedAgentWorkspaceStickySession(workerOne, "worker-1", null)
    ).toBe(workerOne);

    expect(
      resolveNextSelectedAgentWorkspaceStickySession(workerOne, "worker-2", null)
    ).toEqual({ memberId: "worker-2", sessionId: null });

    expect(
      resolveNextSelectedAgentWorkspaceStickySession(workerOne, "", null)
    ).toEqual({ memberId: "", sessionId: null });
  });

  it("clears temporary ACP session overrides after member or runtime catch-up", () => {
    const empty = { memberId: "", sessionId: null as string | null };
    const override = { memberId: "worker-1", sessionId: "runtime-session-2" };

    expect(
      resolveNextSelectedAgentWorkspaceSessionOverride(empty, "worker-1", null)
    ).toBe(empty);
    expect(
      resolveNextSelectedAgentWorkspaceSessionOverride(override, "worker-2", null)
    ).toEqual({ memberId: "", sessionId: null });
    expect(
      resolveNextSelectedAgentWorkspaceSessionOverride(
        override,
        "worker-1",
        " runtime-session-2 "
      )
    ).toEqual({ memberId: "", sessionId: null });
    expect(
      resolveNextSelectedAgentWorkspaceSessionOverride(
        override,
        "worker-1",
        "runtime-session-3"
      )
    ).toBe(override);
  });

  it("refreshes selected member ACP session metadata while an active member has no session id", () => {
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        snapshotStatus: "working",
        agentStatus: "running",
      })
    ).toBe(true);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "member_console",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        runtimeAgentStatus: "running",
      })
    ).toBe(true);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        agentStatus: "WORKING",
      })
    ).toBe(true);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: "session-1",
        snapshotStatus: "working",
      })
    ).toBe(false);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "mailbox",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        snapshotStatus: "working",
      })
    ).toBe(false);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: null,
        selectedSessionId: null,
        snapshotStatus: "working",
      })
    ).toBe(false);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        snapshotStatus: "working",
        runtimeSessionStatus: "stopped",
      })
    ).toBe(false);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        snapshotStatus: "working",
        runtimeAgentStatus: "stopped",
      })
    ).toBe(false);
    expect(
      shouldRefreshSelectedAgentWorkspaceSession({
        activeRunId: "run-1",
        tab: "agent_acp",
        selectedMemberId: "worker-1",
        selectedSessionId: null,
        snapshotStatus: "stopped",
        agentStatus: "stopped",
      })
    ).toBe(false);
  });

  it("extracts positive thread root message ids from conversation payloads", () => {
    expect(resolveThreadRootMessageIdFromPayload(null)).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload("text")).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({})).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({ thread_root_message_id: "7" })).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({ thread_root_message_id: 0 })).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({ thread_root_message_id: -4 })).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({ thread_root_message_id: 7.5 })).toBeNull();
    expect(resolveThreadRootMessageIdFromPayload({ thread_root_message_id: 17 })).toBe(17);
  });

  it("summarizes runtime actions by grouped member operation", () => {
    expect(
      formatTeamRuntimeActionSummary("start", [
        { action: "created" },
        { action: "reused" },
        { action: "created" },
      ])
    ).toBe("Team runtime updated (created=2, reused=1)");
    expect(formatTeamRuntimeActionSummary("stop", [])).toBe("Team runtime stopped");
  });

  it("rejects stale shared-thread requests after team selection changes", () => {
    expect(
      isCurrentTeamScopedRequest({ teamId: "team-1", requestSeq: 3 }, "team-1", 3)
    ).toBe(true);
    expect(
      isCurrentTeamScopedRequest({ teamId: "team-2", requestSeq: 4 }, "team-1", 4)
    ).toBe(false);
    expect(
      isCurrentTeamScopedRequest({ teamId: "team-1", requestSeq: 5 }, "team-1", 4)
    ).toBe(false);
    expect(isCurrentTeamScopedRequest({ teamId: "", requestSeq: 6 }, "", 6)).toBe(false);
  });

  it("validates optional run input JSON", () => {
    expect(validateRunInputJson("   ")).toEqual({
      parsed: undefined,
      error: null,
    });
    expect(validateRunInputJson('{"task":"sync","count":2}')).toEqual({
      parsed: { task: "sync", count: 2 },
      error: null,
    });
    expect(validateRunInputJson("{invalid")).toEqual({
      parsed: undefined,
      error: expect.stringContaining("Run input must be valid JSON"),
    });
  });
});
