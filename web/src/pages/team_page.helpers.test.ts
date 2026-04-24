import { describe, expect, it } from "vitest";

import {
  buildTeamDetailPath,
  buildTeamWorkspacePath,
  formatTeamRuntimeActionSummary,
  parseTeamAgentInputSessionMismatch,
  resolveThreadRootMessageIdFromPayload,
  resolveTeamChannelId,
  resolveTeamSelectedMemberId,
  resolveTeamThreadRootMessageId,
  resolveTeamWorkspaceTab,
  validateRunInputJson,
} from "./team_page";
import { isCurrentTeamScopedRequest } from "./team/page_helpers";

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

  it("builds team detail paths with escaped identifiers", () => {
    expect(buildTeamDetailPath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamDetailPath("team/1")).toBe("/workspace/teams/team%2F1");
  });

  it("builds team workspace paths with optional lens and thread query", () => {
    expect(buildTeamWorkspacePath("team-1")).toBe("/workspace/teams/team-1");
    expect(buildTeamWorkspacePath("team-1", "channels")).toBe(
      "/workspace/teams/team-1?lens=channels"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "review")).toBe(
      "/workspace/teams/team-1?lens=channels&channel=review"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "all", 42)).toBe(
      "/workspace/teams/team-1?lens=channels&thread=42"
    );
    expect(buildTeamWorkspacePath("team-1", "channels", "review", -1)).toBe(
      "/workspace/teams/team-1?lens=channels&channel=review"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, "worker-1", "agent_acp")
    ).toBe("/workspace/teams/team-1?lens=members&member=worker-1&tab=agent_acp");
    expect(buildTeamWorkspacePath("team-1", "members", null, null, " worker-2 ", null)).toBe(
      "/workspace/teams/team-1?lens=members&member=worker-2"
    );
    expect(
      buildTeamWorkspacePath("team-1", "members", null, null, "worker-3", "conversation" as never)
    ).toBe("/workspace/teams/team-1?lens=members&member=worker-3");
  });

  it("parses team channel and thread query state", () => {
    expect(resolveTeamChannelId("")).toBe("all");
    expect(resolveTeamChannelId("?channel=all")).toBe("all");
    expect(resolveTeamChannelId("?channel=research")).toBe("research");
    expect(resolveTeamChannelId("?channel=%C4%B0nbox")).toBe("İnbox");
    expect(resolveTeamThreadRootMessageId("")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=17")).toBe(17);
    expect(resolveTeamThreadRootMessageId("?thread=abc")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=0")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=-8")).toBeNull();
    expect(resolveTeamThreadRootMessageId("?thread=7.5")).toBeNull();
    expect(resolveTeamSelectedMemberId("?member=worker-1")).toBe("worker-1");
    expect(resolveTeamWorkspaceTab("?tab=agent_acp")).toBe("agent_acp");
    expect(resolveTeamWorkspaceTab("?tab=mailbox")).toBe("mailbox");
    expect(resolveTeamWorkspaceTab("?tab=member_console")).toBe("member_console");
    expect(resolveTeamWorkspaceTab("?tab=overview")).toBeNull();
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
