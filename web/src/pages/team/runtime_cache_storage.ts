import type {
  AgentEvent,
  TeamActorMessageRecord,
  TeamConversationMessageRecord,
} from "../../api";
import {
  getLocalStorageItemSafe,
  removeLocalStorageItemSafe,
  setLocalStorageItemSafe,
} from "../../storage/safe_storage";

const STORAGE_KEY = "agenthub_team_runtime_cache_v1";
const STORAGE_VERSION = 1;
const MAX_CONVERSATION_BUCKETS = 16;
const MAX_MEMBER_ACP_BUCKETS = 16;
const MAX_INBOX_BUCKETS = 16;
const CONVERSATION_MESSAGE_LIMIT = 60;
const CONVERSATION_MAILBOX_LIMIT = 40;
const MEMBER_ACP_EVENT_LIMIT = 120;
const INBOX_MESSAGE_LIMIT = 120;

type StoredConversationBucket = {
  updatedAt: number;
  messages: TeamConversationMessageRecord[];
  mailboxMessages: TeamActorMessageRecord[];
};

type StoredMemberAcpBucket = {
  updatedAt: number;
  events: AgentEvent[];
};

type StoredInboxBucket = {
  updatedAt: number;
  messages: TeamActorMessageRecord[];
};

type StoredRuntimeCache = {
  v: number;
  updatedAt: number;
  conversations: Record<string, StoredConversationBucket>;
  memberAcp: Record<string, StoredMemberAcpBucket>;
  inboxes: Record<string, StoredInboxBucket>;
};

type LoadedConversationCache = {
  messages: TeamConversationMessageRecord[];
  mailboxMessages: TeamActorMessageRecord[];
};

function conversationBucketKey(teamId: string, conversationId: string): string {
  return `${teamId}:${conversationId}`;
}

function memberAcpBucketKey(agentId: string, sessionId: string): string {
  return `${agentId}:${sessionId}`;
}

function inboxBucketKey(runId: string, actorId: string): string {
  return `${runId}:${actorId}`;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

function isConversationMessage(value: unknown): value is TeamConversationMessageRecord {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as TeamConversationMessageRecord;
  return (
    isFiniteNumber(candidate.message_id) &&
    typeof candidate.conversation_id === "string" &&
    typeof candidate.from_actor_id === "string" &&
    candidate.payload !== undefined &&
    isFiniteNumber(candidate.created_at)
  );
}

function isMailboxMessage(value: unknown): value is TeamActorMessageRecord {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as TeamActorMessageRecord;
  return (
    isFiniteNumber(candidate.message_id) &&
    typeof candidate.run_id === "string" &&
    typeof candidate.from_actor_id === "string" &&
    typeof candidate.to_actor_id === "string" &&
    typeof candidate.status === "string" &&
    candidate.payload !== undefined &&
    isFiniteNumber(candidate.created_at)
  );
}

function isAgentEvent(value: unknown): value is AgentEvent {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as AgentEvent;
  return (
    isFiniteNumber(candidate.event_id) &&
    typeof candidate.agent_id === "string" &&
    typeof candidate.session_id === "string" &&
    typeof candidate.seq === "string" &&
    typeof candidate.stream === "string" &&
    typeof candidate.message === "string" &&
    isFiniteNumber(candidate.ts)
  );
}

function trimNewestById<T>(
  items: T[],
  limit: number,
  selectId: (item: T) => number
): T[] {
  if (items.length <= limit) {
    return items;
  }
  const next = [...items].sort((left, right) => selectId(left) - selectId(right));
  return next.slice(next.length - limit);
}

function sanitizeConversationMessages(list: unknown): TeamConversationMessageRecord[] {
  if (!Array.isArray(list)) {
    return [];
  }
  return trimNewestById(
    list.filter(isConversationMessage),
    CONVERSATION_MESSAGE_LIMIT,
    (item) => item.message_id
  );
}

function sanitizeMailboxMessages(list: unknown): TeamActorMessageRecord[] {
  if (!Array.isArray(list)) {
    return [];
  }
  return trimNewestById(
    list.filter(isMailboxMessage),
    INBOX_MESSAGE_LIMIT,
    (item) => item.message_id
  );
}

function sanitizeConversationMailboxMessages(list: unknown): TeamActorMessageRecord[] {
  if (!Array.isArray(list)) {
    return [];
  }
  return trimNewestById(
    list.filter(isMailboxMessage),
    CONVERSATION_MAILBOX_LIMIT,
    (item) => item.message_id
  );
}

function sanitizeAgentEvents(list: unknown): AgentEvent[] {
  if (!Array.isArray(list)) {
    return [];
  }
  return trimNewestById(
    list.filter(isAgentEvent),
    MEMBER_ACP_EVENT_LIMIT,
    (item) => item.event_id
  );
}

function limitBuckets<T extends { updatedAt: number }>(
  record: Record<string, T>,
  limit: number
): Record<string, T> {
  const entries = Object.entries(record);
  if (entries.length <= limit) {
    return record;
  }
  return Object.fromEntries(
    entries
      .sort((left, right) => right[1].updatedAt - left[1].updatedAt)
      .slice(0, limit)
  );
}

function loadStorage(): StoredRuntimeCache {
  const raw = getLocalStorageItemSafe(STORAGE_KEY);
  if (!raw) {
    return {
      v: STORAGE_VERSION,
      updatedAt: 0,
      conversations: {},
      memberAcp: {},
      inboxes: {},
    };
  }
  try {
    const parsed = JSON.parse(raw) as Partial<StoredRuntimeCache>;
    if (parsed?.v !== STORAGE_VERSION) {
      return {
        v: STORAGE_VERSION,
        updatedAt: 0,
        conversations: {},
        memberAcp: {},
        inboxes: {},
      };
    }
    const next: StoredRuntimeCache = {
      v: STORAGE_VERSION,
      updatedAt: isFiniteNumber(parsed.updatedAt) ? parsed.updatedAt : 0,
      conversations: {},
      memberAcp: {},
      inboxes: {},
    };
    for (const [key, value] of Object.entries(parsed.conversations ?? {})) {
      next.conversations[key] = {
        updatedAt:
          value && isFiniteNumber((value as { updatedAt?: unknown }).updatedAt)
            ? (value as { updatedAt: number }).updatedAt
            : 0,
        messages: sanitizeConversationMessages(
          (value as { messages?: unknown } | null)?.messages
        ),
        mailboxMessages: sanitizeConversationMailboxMessages(
          (value as { mailboxMessages?: unknown } | null)?.mailboxMessages
        ),
      };
    }
    for (const [key, value] of Object.entries(parsed.memberAcp ?? {})) {
      next.memberAcp[key] = {
        updatedAt:
          value && isFiniteNumber((value as { updatedAt?: unknown }).updatedAt)
            ? (value as { updatedAt: number }).updatedAt
            : 0,
        events: sanitizeAgentEvents((value as { events?: unknown } | null)?.events),
      };
    }
    for (const [key, value] of Object.entries(parsed.inboxes ?? {})) {
      next.inboxes[key] = {
        updatedAt:
          value && isFiniteNumber((value as { updatedAt?: unknown }).updatedAt)
            ? (value as { updatedAt: number }).updatedAt
            : 0,
        messages: sanitizeMailboxMessages((value as { messages?: unknown } | null)?.messages),
      };
    }
    next.conversations = limitBuckets(next.conversations, MAX_CONVERSATION_BUCKETS);
    next.memberAcp = limitBuckets(next.memberAcp, MAX_MEMBER_ACP_BUCKETS);
    next.inboxes = limitBuckets(next.inboxes, MAX_INBOX_BUCKETS);
    return next;
  } catch {
    return {
      v: STORAGE_VERSION,
      updatedAt: 0,
      conversations: {},
      memberAcp: {},
      inboxes: {},
    };
  }
}

function saveStorage(next: StoredRuntimeCache): void {
  const payload: StoredRuntimeCache = {
    ...next,
    updatedAt: Date.now(),
    conversations: limitBuckets(next.conversations, MAX_CONVERSATION_BUCKETS),
    memberAcp: limitBuckets(next.memberAcp, MAX_MEMBER_ACP_BUCKETS),
    inboxes: limitBuckets(next.inboxes, MAX_INBOX_BUCKETS),
  };
  if (
    Object.keys(payload.conversations).length === 0 &&
    Object.keys(payload.memberAcp).length === 0 &&
    Object.keys(payload.inboxes).length === 0
  ) {
    removeLocalStorageItemSafe(STORAGE_KEY);
    return;
  }
  if (!setLocalStorageItemSafe(STORAGE_KEY, JSON.stringify(payload))) {
    removeLocalStorageItemSafe(STORAGE_KEY);
  }
}

export function loadTeamConversationRuntimeCache(
  teamId: string,
  conversationId: string
): LoadedConversationCache {
  const normalizedTeamId = teamId.trim();
  const normalizedConversationId = conversationId.trim();
  if (!normalizedTeamId || !normalizedConversationId) {
    return { messages: [], mailboxMessages: [] };
  }
  const storage = loadStorage();
  const bucket =
    storage.conversations[
      conversationBucketKey(normalizedTeamId, normalizedConversationId)
    ];
  if (!bucket) {
    return { messages: [], mailboxMessages: [] };
  }
  return {
    messages: bucket.messages,
    mailboxMessages: bucket.mailboxMessages,
  };
}

export function saveTeamConversationRuntimeCache(
  teamId: string,
  conversationId: string,
  messages: TeamConversationMessageRecord[],
  mailboxMessages: TeamActorMessageRecord[]
): void {
  const normalizedTeamId = teamId.trim();
  const normalizedConversationId = conversationId.trim();
  if (!normalizedTeamId || !normalizedConversationId) {
    return;
  }
  const storage = loadStorage();
  const key = conversationBucketKey(normalizedTeamId, normalizedConversationId);
  const sanitizedMessages = sanitizeConversationMessages(messages);
  const sanitizedMailboxMessages = sanitizeConversationMailboxMessages(mailboxMessages);
  if (sanitizedMessages.length === 0 && sanitizedMailboxMessages.length === 0) {
    delete storage.conversations[key];
    saveStorage(storage);
    return;
  }
  storage.conversations[key] = {
    updatedAt: Date.now(),
    messages: sanitizedMessages,
    mailboxMessages: sanitizedMailboxMessages,
  };
  saveStorage(storage);
}

export function loadTeamMemberAcpRuntimeCache(
  agentId: string,
  sessionId: string
): AgentEvent[] {
  const normalizedAgentId = agentId.trim();
  const normalizedSessionId = sessionId.trim();
  if (!normalizedAgentId || !normalizedSessionId) {
    return [];
  }
  const storage = loadStorage();
  return (
    storage.memberAcp[memberAcpBucketKey(normalizedAgentId, normalizedSessionId)]?.events ?? []
  );
}

export function saveTeamMemberAcpRuntimeCache(
  agentId: string,
  sessionId: string,
  events: AgentEvent[]
): void {
  const normalizedAgentId = agentId.trim();
  const normalizedSessionId = sessionId.trim();
  if (!normalizedAgentId || !normalizedSessionId) {
    return;
  }
  const storage = loadStorage();
  const key = memberAcpBucketKey(normalizedAgentId, normalizedSessionId);
  const sanitizedEvents = sanitizeAgentEvents(events);
  if (sanitizedEvents.length === 0) {
    delete storage.memberAcp[key];
    saveStorage(storage);
    return;
  }
  storage.memberAcp[key] = {
    updatedAt: Date.now(),
    events: sanitizedEvents,
  };
  saveStorage(storage);
}

export function loadTeamMailboxInboxRuntimeCache(
  runId: string,
  actorId: string
): TeamActorMessageRecord[] {
  const normalizedRunId = runId.trim();
  const normalizedActorId = actorId.trim();
  if (!normalizedRunId || !normalizedActorId) {
    return [];
  }
  const storage = loadStorage();
  return storage.inboxes[inboxBucketKey(normalizedRunId, normalizedActorId)]?.messages ?? [];
}

export function saveTeamMailboxInboxRuntimeCache(
  runId: string,
  actorId: string,
  messages: TeamActorMessageRecord[]
): void {
  const normalizedRunId = runId.trim();
  const normalizedActorId = actorId.trim();
  if (!normalizedRunId || !normalizedActorId) {
    return;
  }
  const storage = loadStorage();
  const key = inboxBucketKey(normalizedRunId, normalizedActorId);
  const sanitizedMessages = sanitizeMailboxMessages(messages);
  if (sanitizedMessages.length === 0) {
    delete storage.inboxes[key];
    saveStorage(storage);
    return;
  }
  storage.inboxes[key] = {
    updatedAt: Date.now(),
    messages: sanitizedMessages,
  };
  saveStorage(storage);
}
