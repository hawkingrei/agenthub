import { describe, expect, it } from "vitest";

import type { TeamActorMessageRecord } from "../../api";
import {
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxPayloadTemplate,
  countUnreadConversationMessages,
  extractMentionedActorIds,
  mergeMailboxMessages,
  resolveConversationMaxMessageId,
  resolveMailboxChatActors,
  selectMailboxConversation,
} from "./mailbox_helpers";

function buildMessage(
  messageId: number,
  overrides: Partial<TeamActorMessageRecord> = {}
): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: "leader",
    to_actor_id: "worker",
    channel: "default",
    transport: "local",
    route: null,
    payload: { type: "chat_message", text: `msg-${messageId}` },
    status: "pending",
    created_at: 1000 + messageId,
    delivered_at: null,
    ...overrides,
  };
}

describe("mailbox helpers", () => {
  it("resolves mailbox chat actors with leader fallback and selection fallback", () => {
    expect(resolveMailboxChatActors("leader", ["leader", "worker"], "worker")).toEqual({
      fromActorId: "leader",
      toActorId: "worker",
      inboxActorId: "worker",
    });

    expect(resolveMailboxChatActors("unknown", ["worker-a", "worker-b"], "unknown")).toEqual({
      fromActorId: "worker-a",
      toActorId: "worker-a",
      inboxActorId: "worker-a",
    });

    expect(resolveMailboxChatActors("leader", [], "worker")).toEqual({
      fromActorId: "",
      toActorId: "",
      inboxActorId: "",
    });
  });

  it("merges mailbox messages with dedup by message id and stable ascending order", () => {
    const recent = [buildMessage(3), buildMessage(1)];
    const inbox = [buildMessage(2), buildMessage(3, { status: "delivered" })];
    const merged = mergeMailboxMessages(recent, inbox);
    expect(merged.map((item) => item.message_id)).toEqual([1, 2, 3]);
    expect(merged[2]?.status).toBe("delivered");
  });

  it("selects conversation messages only for the requested actor pair", () => {
    const messages = [
      buildMessage(1, { from_actor_id: "leader", to_actor_id: "worker" }),
      buildMessage(2, { from_actor_id: "worker", to_actor_id: "leader" }),
      buildMessage(3, { from_actor_id: "leader", to_actor_id: "other" }),
    ];
    expect(selectMailboxConversation(messages, "leader", "worker").map((item) => item.message_id)).toEqual([
      1,
      2,
    ]);
    expect(selectMailboxConversation(messages, "", "worker")).toEqual([]);
  });

  it("builds payload and conversation key deterministically", () => {
    expect(buildMailboxChatPayload("hello")).toEqual({
      type: "chat_message",
      text: "hello",
      source: "team_workbench",
    });
    expect(buildMailboxConversationKey("worker", "leader")).toBe("leader::worker");
    expect(buildMailboxConversationKey("leader", "   ")).toBe("");
  });

  it("extracts unique mentions that match known team members", () => {
    expect(
      extractMentionedActorIds(
        "please check @worker-1 and @worker-2, cc @worker-1 and @unknown",
        ["leader", "worker-1", "worker-2"]
      )
    ).toEqual(["worker-1", "worker-2"]);
    expect(extractMentionedActorIds("plain text", ["worker-1"])).toEqual([]);
  });

  it("builds chat payload with normalized mention ids", () => {
    expect(
      buildMailboxChatPayload("hello @worker-1", {
        mention_actor_ids: ["worker-1", "worker-1", " ", "worker-2"],
      })
    ).toEqual({
      type: "chat_message",
      text: "hello @worker-1",
      source: "team_workbench",
      mention_actor_ids: ["worker-1", "worker-2"],
    });
  });

  it("resolves max message id and unread counts for peer and self conversations", () => {
    const messages = [
      buildMessage(10, { from_actor_id: "leader", to_actor_id: "worker" }),
      buildMessage(11, { from_actor_id: "worker", to_actor_id: "leader" }),
      buildMessage(12, { from_actor_id: "leader", to_actor_id: "leader" }),
      buildMessage(13, { from_actor_id: "leader", to_actor_id: "leader" }),
    ];
    expect(resolveConversationMaxMessageId(messages)).toBe(13);
    expect(resolveConversationMaxMessageId([])).toBeNull();

    expect(countUnreadConversationMessages(messages, "leader", "worker", 10)).toBe(1);
    expect(countUnreadConversationMessages(messages, "leader", "leader", 11)).toBe(2);
    expect(countUnreadConversationMessages(messages, "", "worker", 0)).toBe(0);
  });

  it("returns expected payload templates for all known keys and default branch", () => {
    const keys = [
      "leader_task_assignment",
      "clarification_request",
      "clarification_response",
      "worker_done",
      "worker_blocked",
      "profile_patch_proposal",
    ] as const;
    for (const key of keys) {
      const payload = buildMailboxPayloadTemplate(key);
      expect(payload).not.toEqual({});
    }
    expect(buildMailboxPayloadTemplate("unknown" as never)).toEqual({});
  });
});
