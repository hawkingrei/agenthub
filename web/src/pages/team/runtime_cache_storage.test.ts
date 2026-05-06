import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
} from "../../api";
import {
  loadTeamConversationRuntimeCache,
  loadTeamMailboxInboxRuntimeCache,
  loadTeamMemberAcpRuntimeCache,
  saveTeamConversationRuntimeCache,
  saveTeamMailboxInboxRuntimeCache,
  saveTeamMemberAcpRuntimeCache,
} from "./runtime_cache_storage";

const STORAGE_KEY = "agenthub_team_runtime_cache_v1";

class MemoryStorage implements Storage {
  private readonly store = new Map<string, string>();

  get length(): number {
    return this.store.size;
  }

  clear(): void {
    this.store.clear();
  }

  getItem(key: string): string | null {
    return this.store.get(key) ?? null;
  }

  key(index: number): string | null {
    return [...this.store.keys()][index] ?? null;
  }

  removeItem(key: string): void {
    this.store.delete(key);
  }

  setItem(key: string, value: string): void {
    this.store.set(key, value);
  }
}

function buildConversationMessage(messageId: number): TeamConversationMessageRecord {
  return {
    message_id: messageId,
    conversation_id: "task-all",
    task_id: "task-all",
    from_actor_id: "coordinator",
    to_actor_id: "worker",
    route: "group_chat",
    payload: { type: "chat_message", text: `msg-${messageId}` },
    created_at: messageId,
  };
}

function buildMailboxMessage(messageId: number): TeamActorMessageRecord {
  return {
    message_id: messageId,
    run_id: "run-1",
    from_actor_id: "coordinator",
    from_peer_id: "",
    from_actor_kind: "agent",
    to_actor_id: "worker",
    to_peer_id: "",
    to_actor_kind: "agent",
    channel: "default",
    transport: "local",
    route: null,
    payload: { type: "chat_message", text: `mail-${messageId}` },
    status: "delivered",
    created_at: messageId,
    delivered_at: messageId,
  };
}

function buildAgentEvent(eventId: number): AgentEvent {
  return {
    event_id: eventId,
    agent_id: "agent-1",
    session_id: "session-1",
    seq: String(eventId),
    ts: eventId,
    stream: "acp",
    message: `event-${eventId}`,
  };
}

describe("team runtime cache storage", () => {
  let originalStorage: unknown;

  beforeEach(() => {
    originalStorage = (globalThis as { localStorage?: unknown }).localStorage;
    (globalThis as { localStorage?: unknown }).localStorage = new MemoryStorage();
  });

  afterEach(() => {
    if (originalStorage === undefined) {
      delete (globalThis as { localStorage?: unknown }).localStorage;
      return;
    }
    (globalThis as { localStorage?: unknown }).localStorage = originalStorage;
  });

  it("persists shared-thread messages and mailbox tail by team conversation key", () => {
    saveTeamConversationRuntimeCache(
      " team-1 ",
      " task-all ",
      [buildConversationMessage(1), buildConversationMessage(2)],
      [buildMailboxMessage(10)]
    );

    expect(loadTeamConversationRuntimeCache("team-1", "task-all")).toEqual({
      messages: [buildConversationMessage(1), buildConversationMessage(2)],
      mailboxMessages: [buildMailboxMessage(10)],
    });
  });

  it("trims persisted ACP and inbox caches to the newest entries", () => {
    saveTeamMemberAcpRuntimeCache(
      "agent-1",
      "session-1",
      Array.from({ length: 140 }, (_, index) => buildAgentEvent(index + 1))
    );
    saveTeamMailboxInboxRuntimeCache(
      "run-1",
      "worker",
      Array.from({ length: 140 }, (_, index) => buildMailboxMessage(index + 1))
    );

    const memberEvents = loadTeamMemberAcpRuntimeCache("agent-1", "session-1");
    const inboxMessages = loadTeamMailboxInboxRuntimeCache("run-1", "worker");

    expect(memberEvents).toHaveLength(120);
    expect(memberEvents[0]?.event_id).toBe(21);
    expect(memberEvents[memberEvents.length - 1]?.event_id).toBe(140);
    expect(inboxMessages).toHaveLength(120);
    expect(inboxMessages[0]?.message_id).toBe(21);
    expect(inboxMessages[inboxMessages.length - 1]?.message_id).toBe(140);
  });

  it("trims shared-thread cache buckets to the newest valid entries", () => {
    saveTeamConversationRuntimeCache(
      "team-1",
      "task-all",
      [
        { message_id: Number.NaN },
        ...Array.from({ length: 70 }, (_, index) =>
          buildConversationMessage(index + 1)
        ),
      ] as TeamConversationMessageRecord[],
      [
        { message_id: Number.NaN },
        ...Array.from({ length: 45 }, (_, index) => buildMailboxMessage(index + 1)),
      ] as TeamActorMessageRecord[]
    );

    const cache = loadTeamConversationRuntimeCache("team-1", "task-all");

    expect(cache.messages).toHaveLength(60);
    expect(cache.messages[0]?.message_id).toBe(11);
    expect(cache.messages[cache.messages.length - 1]?.message_id).toBe(70);
    expect(cache.mailboxMessages).toHaveLength(40);
    expect(cache.mailboxMessages[0]?.message_id).toBe(6);
    expect(cache.mailboxMessages[cache.mailboxMessages.length - 1]?.message_id).toBe(
      45
    );
  });

  it("drops empty cache buckets and removes empty storage payloads", () => {
    saveTeamConversationRuntimeCache("team-1", "task-all", [], []);
    saveTeamMemberAcpRuntimeCache("agent-1", "session-1", []);
    saveTeamMailboxInboxRuntimeCache("run-1", "worker", []);

    expect(globalThis.localStorage.getItem(STORAGE_KEY)).toBeNull();
    expect(loadTeamConversationRuntimeCache("", "task-all")).toEqual({
      messages: [],
      mailboxMessages: [],
    });
    expect(loadTeamMemberAcpRuntimeCache("agent-1", " ")).toEqual([]);
    expect(loadTeamMailboxInboxRuntimeCache(" ", "worker")).toEqual([]);
  });

  it("ignores invalid stored payloads and caps stale bucket counts", () => {
    globalThis.localStorage.setItem(STORAGE_KEY, "{invalid");
    expect(loadTeamConversationRuntimeCache("team-1", "task-all")).toEqual({
      messages: [],
      mailboxMessages: [],
    });

    globalThis.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        v: 999,
        updatedAt: 1,
        conversations: {},
        memberAcp: {},
        inboxes: {},
      })
    );
    expect(loadTeamMemberAcpRuntimeCache("agent-1", "session-1")).toEqual([]);

    globalThis.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        v: 1,
        updatedAt: "bad",
        conversations: Object.fromEntries(
          Array.from({ length: 18 }, (_, index) => [
            `team-1:conversation-${index}`,
            {
              updatedAt: index,
              messages: [buildConversationMessage(index + 1)],
              mailboxMessages: [],
            },
          ])
        ),
        memberAcp: {
          "agent-1:session-1": {
            updatedAt: "bad",
            events: [buildAgentEvent(1), { event_id: Number.NaN }],
          },
        },
        inboxes: {
          "run-1:worker": {
            updatedAt: "bad",
            messages: [buildMailboxMessage(1), { message_id: Number.NaN }],
          },
        },
      })
    );

    expect(loadTeamConversationRuntimeCache("team-1", "conversation-0")).toEqual({
      messages: [],
      mailboxMessages: [],
    });
    expect(loadTeamConversationRuntimeCache("team-1", "conversation-17").messages).toEqual([
      buildConversationMessage(18),
    ]);
    expect(loadTeamMemberAcpRuntimeCache("agent-1", "session-1")).toEqual([
      buildAgentEvent(1),
    ]);
    expect(loadTeamMailboxInboxRuntimeCache("run-1", "worker")).toEqual([
      buildMailboxMessage(1),
    ]);
  });
});
