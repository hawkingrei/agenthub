import { describe, expect, it } from "vitest";

import type { TeamActorMessageRecord } from "../../api";
import {
  applyMentionAtTag,
  buildMailboxChatPayload,
  buildMailboxConversationKey,
  buildMailboxForwardChatPayload,
  buildMailboxPayloadTemplate,
  canonicalizeMentionDraft,
  createDisplayNameLookup,
  countUnreadConversationMessages,
  extractMentionedActorIds,
  mergeMailboxMessages,
  renderMarkdownWithMentions,
  renderPlainTextWithMentions,
  resolveChatMessageText,
  resolveDisplayName,
  resolveMentionDraftQuery,
  resolveTaskMailboxRoutePlan,
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

  it("extracts unique <at> mentions that match known team members", () => {
    expect(
      extractMentionedActorIds(
        "please check <at>worker-1</at> and <at>worker-2</at>, cc <at>worker-1</at>",
        ["leader", "worker-1", "worker-2"]
      )
    ).toEqual(["worker-1", "worker-2"]);
    expect(extractMentionedActorIds("plain text", ["worker-1"])).toEqual([]);
    expect(
      extractMentionedActorIds(
        "assign <at>worker-2</at> and mail worker@company.com @worker-1",
        ["worker-1", "worker-2"]
      )
    ).toEqual(["worker-2"]);
  });

  it("resolves mention query around cursor and avoids email-like cases", () => {
    expect(resolveMentionDraftQuery("ping @work", 10)).toEqual({
      start: 5,
      end: 10,
      keyword: "work",
    });
    expect(resolveMentionDraftQuery("mail user@corp.com", 15)).toBeNull();
    expect(resolveMentionDraftQuery("plain text", 5)).toBeNull();
  });

  it("applies selected member as @ mention token", () => {
    const mention = resolveMentionDraftQuery("please check @work soon", 18);
    expect(mention).not.toBeNull();
    const applied = applyMentionAtTag("please check @work soon", mention!, "worker-1");
    expect(applied.text).toBe("please check @worker-1 soon");
    expect(applied.cursor).toBe("please check @worker-1".length);
  });

  it("canonicalizes mentions without dropping trailing text", () => {
    expect(
      canonicalizeMentionDraft("please continue @Worker Agent and review", [
        {
          actorId: "worker-agent",
          label: "Worker Agent",
          aliases: ["Worker Agent"],
        },
      ])
    ).toEqual({
      text: "please continue <at>worker-agent</at> and review",
      mentionActorIds: ["worker-agent"],
    });
  });

  it("canonicalizes mentions followed by punctuation without truncating punctuation", () => {
    expect(
      canonicalizeMentionDraft(
        "please continue @Worker Agent, then escalate @Worker Agent:",
        [
          {
            actorId: "worker-agent",
            label: "Worker Agent",
            aliases: ["Worker Agent"],
          },
        ]
      )
    ).toEqual({
      text:
        "please continue <at>worker-agent</at>, then escalate <at>worker-agent</at>:",
      mentionActorIds: ["worker-agent"],
    });
  });

  it("does not tokenize raw mentions inside url-like plain text", () => {
    const rendered = renderPlainTextWithMentions(
      "see https://example.com/@worker-1 and /tmp/@worker-2"
    );
    expect(rendered).not.toContain("team-mention");
    expect(rendered).toContain("https://example.com/@worker-1");
    expect(rendered).toContain("/tmp/@worker-2");
  });

  it("still tokenizes raw mentions after whitespace in plain text", () => {
    const rendered = renderPlainTextWithMentions("hello @worker-1");
    expect(rendered).toContain("team-mention");
    expect(rendered).toContain("@worker-1");
  });

  it("renders <at> mention as visual chip in markdown/plain text output", () => {
    const markdown = renderMarkdownWithMentions("hello <at>worker-1</at>");
    expect(markdown).toContain("team-mention");
    expect(markdown).toContain("@worker-1");

    const plain = renderPlainTextWithMentions("hello <at>worker-1</at>");
    expect(plain).toContain("team-mention");
    expect(plain).toContain("@worker-1");
  });

  it("treats plain string mailbox payloads as chat text", () => {
    expect(resolveChatMessageText("line one\n\n- line two")).toBe("line one\n\n- line two");
  });

  it("renders raw mentions as chips in plain text only", () => {
    const plain = renderPlainTextWithMentions("hello @worker-1");
    expect(plain).toContain("team-mention");
    expect(plain).toContain("@worker-1");

    const markdown = renderMarkdownWithMentions("hello @worker-1");
    expect(markdown).not.toContain("team-mention");
    expect(markdown).toContain("@worker-1");
  });

  it("does not convert raw mentions inside markdown code spans or links into chips", () => {
    const rendered = renderMarkdownWithMentions(
      "`@worker-1` and [profile](https://example.com/@worker-1)"
    );
    expect(rendered).not.toContain("team-mention");
    expect(rendered).toContain("@worker-1");
    expect(rendered).toContain("https://example.com/@worker-1");
  });

  it("uses null-prototype lookups and falls back safely for reserved property names", () => {
    const lookup = createDisplayNameLookup([
      ["worker-1", "Worker One"],
      ["toString", "String Agent"],
    ]);
    expect(Object.getPrototypeOf(lookup)).toBeNull();
    expect(resolveDisplayName("worker-1", lookup)).toBe("Worker One");
    expect(resolveDisplayName("toString", lookup)).toBe("String Agent");
    expect(resolveDisplayName("valueOf", lookup, "valueOf")).toBe("valueOf");
  });

  it("renders mention chips with safe display-name lookup values only", () => {
    const markdown = renderMarkdownWithMentions("hello <at>toString</at>", {
      worker: "Worker",
    });
    expect(markdown).toContain("@toString");
    expect(markdown).not.toContain("function toString");
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

  it("builds summary-first chat payload when detail_ref is present", () => {
    expect(
      buildMailboxChatPayload("Concise summary for the peer", {
        detail_ref: {
          uri: "artifact://team/task-all/evidence-1",
          label: "full evidence",
          kind: "artifact",
          content_type: "application/json",
        },
      })
    ).toEqual({
      type: "chat_message",
      text: "Concise summary for the peer",
      source: "team_workbench",
      summary: "Concise summary for the peer",
      detail_ref: {
        uri: "artifact://team/task-all/evidence-1",
        label: "full evidence",
        kind: "artifact",
        content_type: "application/json",
      },
    });
  });

  it("translates mailbox address into @ mention when forwarding without explicit mentions", () => {
    const broadcastPayload = buildMailboxChatPayload("please check logs");
    expect(buildMailboxForwardChatPayload(broadcastPayload, "worker-1")).toEqual({
      type: "chat_message",
      text: "@worker-1 please check logs",
      source: "team_workbench",
      mention_actor_ids: ["worker-1"],
    });

    const explicitMentionPayload = buildMailboxChatPayload("@worker-2 @worker-1 check logs", {
      mention_actor_ids: ["worker-2", "worker-1", "worker-2"],
    });
    expect(buildMailboxForwardChatPayload(explicitMentionPayload, "worker-1")).toEqual({
      type: "chat_message",
      text: "@worker-2 @worker-1 check logs",
      source: "team_workbench",
      mention_actor_ids: ["worker-1"],
    });

    const mismatchedMentionPayload = buildMailboxChatPayload("@worker-2 check logs", {
      mention_actor_ids: ["worker-2"],
    });
    expect(buildMailboxForwardChatPayload(mismatchedMentionPayload, "worker-1")).toEqual({
      type: "chat_message",
      text: "@worker-1 @worker-2 check logs",
      source: "team_workbench",
      mention_actor_ids: ["worker-1"],
    });

    expect(buildMailboxForwardChatPayload(explicitMentionPayload, " ")).toEqual({
      type: "chat_message",
      text: "@worker-2 @worker-1 check logs",
      source: "team_workbench",
      mention_actor_ids: ["worker-2", "worker-1"],
    });
  });

  it("resolves task mailbox route plan for mention and broadcast modes", () => {
    expect(
      resolveTaskMailboxRoutePlan(
        ["leader", "worker-1", "worker-2"],
        ["worker-2", "worker-1", "worker-2", "unknown"],
        "leader"
      )
    ).toEqual({
      fromActorId: "leader",
      toActorIds: ["worker-2", "worker-1"],
    });

    expect(
      resolveTaskMailboxRoutePlan(
        ["worker-1", "leader", "worker-2"],
        [],
        "leader"
      )
    ).toEqual({
      fromActorId: "leader",
      toActorIds: ["leader", "worker-1", "worker-2"],
    });

    expect(resolveTaskMailboxRoutePlan(["worker-1"], [], "missing")).toEqual({
      fromActorId: "worker-1",
      toActorIds: ["worker-1"],
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
      if (key === "profile_patch_proposal") {
        expect(payload).toMatchObject({
          type: "profile_patch_proposal",
          description: expect.any(String),
        });
      }
    }
    expect(buildMailboxPayloadTemplate("unknown" as never)).toEqual({});
  });
});
