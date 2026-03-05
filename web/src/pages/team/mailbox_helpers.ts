import { TeamActorMessageRecord } from "../../api";

export type MailboxTemplateKey =
  | "leader_task_assignment"
  | "clarification_request"
  | "clarification_response"
  | "worker_done"
  | "worker_blocked"
  | "profile_patch_proposal";

export type TeamMailboxChatActors = {
  fromActorId: string;
  toActorId: string;
  inboxActorId: string;
};

export type TaskMailboxRoutePlan = {
  fromActorId: string;
  toActorIds: string[];
};

const MENTION_TOKEN_REGEX = /@([A-Za-z0-9._:-]+)/g;

function normalizeActorIds(actorIds: string[]): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const rawActorId of actorIds) {
    const actorId = rawActorId.trim();
    if (!actorId || seen.has(actorId)) {
      continue;
    }
    seen.add(actorId);
    normalized.push(actorId);
  }
  return normalized;
}

export function resolveMailboxChatActors(
  leaderMemberId: string | null | undefined,
  memberIds: string[],
  selectedMemberId: string
): TeamMailboxChatActors {
  if (memberIds.length === 0) {
    return {
      fromActorId: "",
      toActorId: "",
      inboxActorId: "",
    };
  }
  const normalizedLeaderId = (leaderMemberId ?? "").trim();
  const leaderId = normalizedLeaderId && memberIds.includes(normalizedLeaderId)
    ? normalizedLeaderId
    : memberIds[0] ?? "";
  const normalizedSelectedId = selectedMemberId.trim();
  const targetId = normalizedSelectedId && memberIds.includes(normalizedSelectedId)
    ? normalizedSelectedId
    : memberIds[0] ?? "";
  return {
    fromActorId: leaderId,
    toActorId: targetId,
    inboxActorId: targetId,
  };
}

export function mergeMailboxMessages(
  recentMessages: TeamActorMessageRecord[],
  inboxMessages: TeamActorMessageRecord[]
): TeamActorMessageRecord[] {
  const byId = new Map<number, TeamActorMessageRecord>();
  for (const message of [...recentMessages, ...inboxMessages]) {
    byId.set(message.message_id, message);
  }
  return [...byId.values()].sort((a, b) => a.message_id - b.message_id);
}

export function selectMailboxConversation(
  messages: TeamActorMessageRecord[],
  actorA: string,
  actorB: string
): TeamActorMessageRecord[] {
  const left = actorA.trim();
  const right = actorB.trim();
  if (!left || !right) {
    return [];
  }
  return messages.filter(
    (message) =>
      (message.from_actor_id === left && message.to_actor_id === right) ||
      (message.from_actor_id === right && message.to_actor_id === left)
  );
}

export function extractMentionedActorIds(text: string, memberIds: string[]): string[] {
  const normalizedMembers = new Set(
    memberIds.map((memberId) => memberId.trim()).filter((memberId) => memberId.length > 0)
  );
  if (normalizedMembers.size === 0) {
    return [];
  }
  const out: string[] = [];
  const seen = new Set<string>();
  for (const match of text.matchAll(MENTION_TOKEN_REGEX)) {
    const actorId = (match[1] ?? "").trim();
    if (!actorId || !normalizedMembers.has(actorId) || seen.has(actorId)) {
      continue;
    }
    seen.add(actorId);
    out.push(actorId);
  }
  return out;
}

export function resolveTaskMailboxRoutePlan(
  memberIds: string[],
  mentionActorIds: string[],
  leaderMemberId?: string | null
): TaskMailboxRoutePlan {
  const normalizedMembers = normalizeActorIds(memberIds);
  if (normalizedMembers.length === 0) {
    return {
      fromActorId: "",
      toActorIds: [],
    };
  }

  const memberSet = new Set(normalizedMembers);
  const normalizedMentions = normalizeActorIds(mentionActorIds).filter((actorId) =>
    memberSet.has(actorId)
  );
  const normalizedLeaderId = (leaderMemberId ?? "").trim();
  const fromActorId =
    normalizedLeaderId && memberSet.has(normalizedLeaderId)
      ? normalizedLeaderId
      : (normalizedMembers[0] ?? "");

  if (!fromActorId) {
    return {
      fromActorId: "",
      toActorIds: [],
    };
  }
  if (normalizedMentions.length > 0) {
    return {
      fromActorId,
      toActorIds: normalizedMentions,
    };
  }

  return {
    fromActorId,
    toActorIds: [
      fromActorId,
      ...normalizedMembers.filter((memberId) => memberId !== fromActorId),
    ],
  };
}

type MailboxChatPayload = {
  type: "chat_message";
  text: string;
  source: "team_workbench";
  mention_actor_ids?: string[];
};

export function buildMailboxChatPayload(
  text: string,
  options?: { mention_actor_ids?: string[] }
): MailboxChatPayload {
  const mentionActorIds = (options?.mention_actor_ids ?? [])
    .map((actorId) => actorId.trim())
    .filter((actorId, index, list) => actorId.length > 0 && list.indexOf(actorId) === index);
  const payload: MailboxChatPayload = {
    type: "chat_message",
    text,
    source: "team_workbench",
  };
  if (mentionActorIds.length > 0) {
    payload.mention_actor_ids = mentionActorIds;
  }
  return payload;
}

export function buildMailboxForwardChatPayload(
  basePayload: MailboxChatPayload,
  toActorId: string
): MailboxChatPayload {
  const targetActorId = toActorId.trim();
  const explicitMentions = normalizeActorIds(basePayload.mention_actor_ids ?? []);
  const normalizedText = basePayload.text.trim();
  if (!targetActorId) {
    if (explicitMentions.length > 0) {
      return { ...basePayload, mention_actor_ids: explicitMentions };
    }
    return basePayload;
  }
  const mentionToken = `@${targetActorId}`;
  const textWithMention = normalizedText.includes(mentionToken)
    ? normalizedText
    : `${mentionToken} ${normalizedText}`.trim();
  if (explicitMentions.length > 0) {
    return {
      ...basePayload,
      text: textWithMention,
      mention_actor_ids: [targetActorId],
    };
  }
  return {
    ...basePayload,
    text: textWithMention,
    mention_actor_ids: [targetActorId],
  };
}

export function buildMailboxConversationKey(actorA: string, actorB: string): string {
  const pair = [actorA.trim(), actorB.trim()].filter((value) => value.length > 0).sort();
  if (pair.length < 2) {
    return "";
  }
  return `${pair[0]}::${pair[1]}`;
}

export function resolveConversationMaxMessageId(
  messages: TeamActorMessageRecord[]
): number | null {
  if (messages.length === 0) {
    return null;
  }
  return messages.reduce(
    (maxId, message) => (message.message_id > maxId ? message.message_id : maxId),
    messages[0]?.message_id ?? 0
  );
}

export function countUnreadConversationMessages(
  messages: TeamActorMessageRecord[],
  actorA: string,
  actorB: string,
  seenMessageId: number
): number {
  const left = actorA.trim();
  const right = actorB.trim();
  if (!left || !right) {
    return 0;
  }
  return messages.filter((message) => {
    if (message.message_id <= seenMessageId) {
      return false;
    }
    const inConversation =
      (message.from_actor_id === left && message.to_actor_id === right) ||
      (message.from_actor_id === right && message.to_actor_id === left);
    if (!inConversation) {
      return false;
    }
    if (left === right) {
      return true;
    }
    return message.to_actor_id === left;
  }).length;
}

export function buildMailboxPayloadTemplate(template: MailboxTemplateKey): unknown {
  switch (template) {
    case "leader_task_assignment":
      return {
        type: "leader_task_assignment",
        task: "Implement the requested change in a focused scope.",
        acceptance: "All listed checks pass and artifacts are updated.",
        deadline: "asap",
      };
    case "clarification_request":
      return {
        type: "clarification_request",
        question: "Need one product decision before continuing.",
        choices: ["option_a", "option_b"],
        blocking_scope: "run",
        context: {},
      };
    case "clarification_response":
      return {
        type: "clarification_response",
        request_id: "fill_request_id",
        answer: "option_a",
        rationale: "Fits current constraints and priority.",
      };
    case "worker_done":
      return {
        type: "worker_status",
        status: "done",
        result: "Implemented scoped change and verified behavior.",
        evidence: ["path/to/file:123", "test_name"],
      };
    case "worker_blocked":
      return {
        type: "worker_status",
        status: "blocked",
        result: "Blocked by missing requirement detail.",
        evidence: ["blocking_input_missing"],
        next_action: "Please provide target behavior for edge case X.",
      };
    case "profile_patch_proposal":
      return {
        type: "profile_patch_proposal",
        target: "run",
        prompt_append: "Add missing domain constraint and output contract.",
        skills_add: ["team-leader-orchestrator"],
      };
    default:
      return {};
  }
}
