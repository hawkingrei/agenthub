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
    from_actor_id: "leader",
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
    from_actor_id: "leader",
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
      "team-1",
      "task-all",
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
});
