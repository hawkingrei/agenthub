import { TeamActorMessageRecord } from "../../api";
import { renderTeamMarkdownCached } from "./team_markdown";
import { escapeTeamHtml } from "./team_text_helpers";

export type MailboxTemplateKey =
  | "coordinator_task_assignment"
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

const MENTION_TAG_REGEX = /<at>\s*([A-Za-z0-9._:-]+)\s*<\/at>/gi;
const MARKDOWN_CODE_FENCE_PATTERN = /^\s{0,3}(```|~~~)/m;
const MARKDOWN_HEADING_PATTERN = /^\s{0,3}#{1,6}\s+/m;
const MARKDOWN_BLOCKQUOTE_PATTERN = /^\s{0,3}>\s+/m;
const MARKDOWN_LINK_PATTERN = /!?\[[^\]]+\]\(([^)]+)\)/;
const MARKDOWN_AUTOLINK_PATTERN = /<[a-z][a-z0-9+.-]{1,31}:[^>\s]+>/i;
const MARKDOWN_TABLE_PATTERN = /^\|.+\|\s*$/m;
const MARKDOWN_HORIZONTAL_RULE_PATTERN = /^\s{0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/m;
const MARKDOWN_INLINE_STYLE_PATTERN = /(?:\*\*[^*\n]+\*\*|\*[^*\n]+\*|__[^_\n]+__|_[^_\n]+_|~~[^~\n]+~~)/;
const MARKDOWN_INLINE_CODE_PATTERN = /`[^`\n]+`/;
const SHORT_CHAT_LIST_ITEM_PATTERN = /^\s*(?:[-*+]|\d+\.)\s+(.+)\s*$/;
export type MentionDraftQuery = {
  start: number;
  end: number;
  keyword: string;
};

export type MentionCandidate = {
  actorId: string;
  label: string;
  aliases: string[];
};

export function resolveDisplayName(
  actorId: string,
  displayNameByActorId?: Record<string, string>,
  fallback?: string
): string {
  const normalizedActorId = actorId.trim();
  if (!normalizedActorId) {
    return fallback ?? "-";
  }
  const candidate = displayNameByActorId?.[normalizedActorId];
  return typeof candidate === "string" && candidate.trim().length > 0
    ? candidate.trim()
    : (fallback ?? normalizedActorId);
}

export function createDisplayNameLookup(
  entries: Iterable<[string, string]>
): Record<string, string> {
  const lookup: Record<string, string> = Object.create(null) as Record<string, string>;
  for (const [rawActorId, rawLabel] of entries) {
    const actorId = rawActorId.trim();
    const label = rawLabel.trim();
    if (!actorId || !label) {
      continue;
    }
    lookup[actorId] = label;
  }
  return lookup;
}

export function normalizeRawMentionActorId(rawActorId: string): string {
  return rawActorId.trim().replace(/\.+$/, "");
}

export function isHumanMailboxActor(
  actorId: string | null | undefined,
  humanActorId: string
): boolean {
  const normalizedActorId = (actorId ?? "").trim();
  const normalizedHumanActorId = humanActorId.trim();
  if (!normalizedActorId || !normalizedHumanActorId) {
    return false;
  }
  return (
    normalizedActorId === normalizedHumanActorId ||
    normalizedActorId.startsWith(`${normalizedHumanActorId}:`)
  );
}

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
  coordinatorMemberId: string | null | undefined,
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
  const normalizedCoordinatorId = (coordinatorMemberId ?? "").trim();
  const coordinatorId = normalizedCoordinatorId && memberIds.includes(normalizedCoordinatorId)
    ? normalizedCoordinatorId
    : memberIds[0] ?? "";
  const normalizedSelectedId = selectedMemberId.trim();
  const targetId = normalizedSelectedId && memberIds.includes(normalizedSelectedId)
    ? normalizedSelectedId
    : memberIds[0] ?? "";
  return {
    fromActorId: coordinatorId,
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
  for (const match of text.matchAll(MENTION_TAG_REGEX)) {
    const actorId = (match[1] ?? "").trim();
    if (!actorId || !normalizedMembers.has(actorId) || seen.has(actorId)) {
      continue;
    }
    seen.add(actorId);
    out.push(actorId);
  }
  return out;
}

export function resolveMentionDraftQuery(
  draft: string,
  cursorPosition: number
): MentionDraftQuery | null {
  const cursor = Math.max(0, Math.min(cursorPosition, draft.length));
  const atIndex = draft.lastIndexOf("@", Math.max(0, cursor - 1));
  if (atIndex < 0 || atIndex >= cursor) {
    return null;
  }
  const previous = atIndex > 0 ? draft.charAt(atIndex - 1) : "";
  if (/[A-Za-z0-9._%+-]/.test(previous)) {
    return null;
  }
  const keyword = draft.slice(atIndex + 1, cursor);
  if (keyword.includes("<") || keyword.includes(">") || /\s/.test(keyword)) {
    return null;
  }
  if (!/^[A-Za-z0-9._:-]*$/.test(keyword)) {
    return null;
  }
  return {
    start: atIndex,
    end: cursor,
    keyword,
  };
}

export function applyMentionAtTag(
  draft: string,
  mention: MentionDraftQuery,
  label: string
): { text: string; cursor: number } {
  const normalizedLabel = label.trim();
  if (!normalizedLabel) {
    return { text: draft, cursor: mention.end };
  }
  const prefix = draft.slice(0, mention.start);
  const suffix = draft.slice(mention.end);
  const tag = `@${normalizedLabel}`;
  const needsSpace = suffix.length === 0 || /^\s/.test(suffix) ? "" : " ";
  const nextText = `${prefix}${tag}${needsSpace}${suffix}`;
  const nextCursor = prefix.length + tag.length + needsSpace.length;
  return {
    text: nextText,
    cursor: nextCursor,
  };
}

function normalizeMentionCandidates(candidates: MentionCandidate[]): MentionCandidate[] {
  const seenActorIds = new Set<string>();
  const normalized: MentionCandidate[] = [];
  for (const candidate of candidates) {
    const actorId = candidate.actorId.trim();
    const label = candidate.label.trim();
    if (!actorId || !label || seenActorIds.has(actorId)) {
      continue;
    }
    seenActorIds.add(actorId);
    normalized.push({
      actorId,
      label,
      aliases: normalizeActorIds([label, actorId, ...(candidate.aliases ?? [])]).sort(
        (left, right) => right.length - left.length
      ),
    });
  }
  return normalized;
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

export function canonicalizeMentionDraft(
  draft: string,
  candidates: MentionCandidate[]
): { text: string; mentionActorIds: string[] } {
  let nextText = draft;
  const mentionActorIds: string[] = [];
  const seen = new Set<string>();
  const normalizedCandidates = normalizeMentionCandidates(candidates).sort(
    (left, right) =>
      Math.max(...right.aliases.map((alias) => alias.length)) -
      Math.max(...left.aliases.map((alias) => alias.length))
  );

  for (const candidate of normalizedCandidates) {
    for (const alias of candidate.aliases) {
      if (!alias) {
        continue;
      }
      const pattern = new RegExp(
        `(^|[^A-Za-z0-9._%+-])@${escapeRegex(alias)}(?=$|[^A-Za-z0-9._:-]|:(?=[^A-Za-z0-9._-]|$))`,
        "g"
      );
      nextText = nextText.replace(pattern, (_match, prefix: string) => {
        if (!seen.has(candidate.actorId)) {
          seen.add(candidate.actorId);
          mentionActorIds.push(candidate.actorId);
        }
        return `${prefix}<at>${candidate.actorId}</at>`;
      });
    }
  }

  return {
    text: nextText,
    mentionActorIds,
  };
}

export function resolveTaskMailboxRoutePlan(
  memberIds: string[],
  mentionActorIds: string[],
  coordinatorMemberId?: string | null
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
  const normalizedCoordinatorId = (coordinatorMemberId ?? "").trim();
  const fromActorId =
    normalizedCoordinatorId && memberSet.has(normalizedCoordinatorId)
      ? normalizedCoordinatorId
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
  summary?: string;
  detail_ref?: {
    uri: string;
    label?: string;
    kind?: string;
    content_type?: string;
  };
};

type MailboxChatDetailRefInput =
  | string
  | {
      uri: string;
      label?: string;
      kind?: string;
      content_type?: string;
    };

export function parseStructuredTeamPayload(payload: unknown): unknown {
  if (typeof payload !== "string") {
    return payload;
  }
  const trimmed = payload.trim();
  if (!trimmed.startsWith("{") && !trimmed.startsWith("[")) {
    return payload;
  }
  try {
    return JSON.parse(trimmed) as unknown;
  } catch {
    return payload;
  }
}

function resolveStructuredPayloadTextByType(parsed: unknown, expectedType: string): string | null {
  if (expectedType === "chat_message" && typeof parsed === "string") {
    return parsed;
  }
  if (
    typeof parsed === "object" &&
    parsed !== null &&
    "type" in parsed &&
    (parsed as { type?: unknown }).type === expectedType &&
    "text" in parsed
  ) {
    return String((parsed as { text?: unknown }).text ?? "");
  }
  return null;
}

export function resolveChatMessageText(payload: unknown): string | null {
  return resolveStructuredPayloadTextByType(parseStructuredTeamPayload(payload), "chat_message");
}

export function resolveVisibleTeamPayloadText(payload: unknown): string | null {
  const parsed = parseStructuredTeamPayload(payload);
  const chatText = resolveStructuredPayloadTextByType(parsed, "chat_message");
  if (chatText !== null) {
    return chatText;
  }
  return resolveStructuredPayloadTextByType(parsed, "task_note");
}

export function buildMailboxChatPayload(
  text: string,
  options?: {
    mention_actor_ids?: string[];
    summary?: string;
    detail_ref?: MailboxChatDetailRefInput;
  }
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
  const detailRef =
    typeof options?.detail_ref === "string"
      ? { uri: options.detail_ref.trim() }
      : options?.detail_ref
        ? {
            uri: options.detail_ref.uri.trim(),
            label: options.detail_ref.label?.trim() || undefined,
            kind: options.detail_ref.kind?.trim() || undefined,
            content_type: options.detail_ref.content_type?.trim() || undefined,
          }
        : null;
  if (detailRef?.uri) {
    payload.detail_ref = detailRef;
    const summary = (options?.summary ?? text).trim();
    if (summary) {
      payload.summary = summary;
    }
  } else if (options?.summary?.trim()) {
    payload.summary = options.summary.trim();
  }
  return payload;
}

function mentionChipHtml(actorId: string, displayNameByActorId?: Record<string, string>): string {
  const label = resolveDisplayName(actorId, displayNameByActorId, actorId);
  const escapedActorId = escapeTeamHtml(actorId);
  return `<button type="button" class="team-mention inline-flex items-center rounded-md border border-brand-primary/40 bg-brand-primary/10 px-1.5 py-0.5 text-[11px] text-brand-primary transition hover:bg-brand-primary/15" data-team-agent-mention-id="${escapedActorId}">@${escapeTeamHtml(label)}</button>`;
}

function isRawMentionBoundary(previous: string): boolean {
  return previous.length === 0 || /[\s([{'"`]/.test(previous);
}

function replaceRawMentionsWithTokens(text: string): string {
  const chunks: string[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    if (text.charAt(cursor) !== "@") {
      chunks.push(text.charAt(cursor));
      cursor += 1;
      continue;
    }
    const previous = cursor > 0 ? text.charAt(cursor - 1) : "";
    if (!isRawMentionBoundary(previous)) {
      chunks.push("@");
      cursor += 1;
      continue;
    }
    let end = cursor + 1;
    while (end < text.length && /[A-Za-z0-9._:-]/.test(text.charAt(end))) {
      end += 1;
    }
    if (end === cursor + 1) {
      chunks.push("@");
      cursor += 1;
      continue;
    }
    const rawActorId = text.slice(cursor + 1, end).trim();
    const actorId = normalizeRawMentionActorId(rawActorId);
    if (!actorId) {
      chunks.push(`@${rawActorId}`);
      cursor = end;
      continue;
    }
    chunks.push(`%%AGH_AT_MENTION:${actorId}%%${rawActorId.slice(actorId.length)}`);
    cursor = end;
  }
  return chunks.join("");
}

function collectMarkdownMentionProtectedRanges(text: string): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];
  const inlineCodePattern = /`[^`\n]+`/g;
  const markdownLinkPattern = /!?\[[^\]]*]\([^)]+\)/g;
  for (const pattern of [inlineCodePattern, markdownLinkPattern]) {
    for (const match of text.matchAll(pattern)) {
      if (typeof match.index === "number") {
        ranges.push([match.index, match.index + match[0].length]);
      }
    }
  }
  return ranges.sort((left, right) => left[0] - right[0] || right[1] - left[1]);
}

function replaceRawMentionsOutsideMarkdownProtectedRanges(text: string): string {
  const ranges = collectMarkdownMentionProtectedRanges(text);
  if (ranges.length === 0) {
    return replaceRawMentionsWithTokens(text);
  }
  const chunks: string[] = [];
  let cursor = 0;
  for (const [start, end] of ranges) {
    if (start < cursor) {
      continue;
    }
    if (cursor < start) {
      const prefix = text.slice(cursor, start);
      const contextPrefix = cursor > 0 ? text.charAt(cursor - 1) : "";
      const tokenized = replaceRawMentionsWithTokens(`${contextPrefix}${prefix}`);
      chunks.push(cursor > 0 ? tokenized.slice(contextPrefix.length) : tokenized);
    }
    chunks.push(text.slice(start, end));
    cursor = end;
  }
  if (cursor < text.length) {
    const suffix = text.slice(cursor);
    const contextPrefix = cursor > 0 ? text.charAt(cursor - 1) : "";
    const tokenized = replaceRawMentionsWithTokens(`${contextPrefix}${suffix}`);
    chunks.push(cursor > 0 ? tokenized.slice(contextPrefix.length) : tokenized);
  }
  return chunks.join("");
}

function replaceCanonicalMentionsWithTokens(text: string): string {
  return text.replace(MENTION_TAG_REGEX, (_match, rawActorId: string) => {
    const actorId = (rawActorId ?? "").trim();
    if (!/^[A-Za-z0-9._:-]+$/.test(actorId)) {
      return "";
    }
    return `%%AGH_AT_MENTION:${actorId}%%`;
  });
}

function tokenizePlainTextMentions(text: string): string {
  return replaceCanonicalMentionsWithTokens(replaceRawMentionsWithTokens(text));
}

function renderMentionTokensIntoHtml(
  text: string,
  displayNameByActorId?: Record<string, string>
): string {
  return text.replace(/%%AGH_AT_MENTION:([A-Za-z0-9._:-]+)%%/g, (_match, actorId: string) =>
    mentionChipHtml(actorId, displayNameByActorId)
  );
}

function renderCanonicalPlainTextWithMentions(
  text: string,
  displayNameByActorId?: Record<string, string>
): string {
  const tokenized = replaceCanonicalMentionsWithTokens(text);
  const escaped = escapeTeamHtml(tokenized).replace(/\n/g, "<br/>");
  return renderMentionTokensIntoHtml(escaped, displayNameByActorId);
}

function hasExplicitStructuredMarkdown(text: string): boolean {
  return (
    MARKDOWN_CODE_FENCE_PATTERN.test(text) ||
    MARKDOWN_HEADING_PATTERN.test(text) ||
    MARKDOWN_BLOCKQUOTE_PATTERN.test(text) ||
    MARKDOWN_LINK_PATTERN.test(text) ||
    MARKDOWN_AUTOLINK_PATTERN.test(text) ||
    MARKDOWN_TABLE_PATTERN.test(text) ||
    MARKDOWN_HORIZONTAL_RULE_PATTERN.test(text) ||
    MARKDOWN_INLINE_STYLE_PATTERN.test(text) ||
    MARKDOWN_INLINE_CODE_PATTERN.test(text)
  );
}

function isShortListLikeChat(text: string): boolean {
  const lines = text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
  if (lines.length < 2) {
    return false;
  }
  const items = lines.map((line) => line.match(SHORT_CHAT_LIST_ITEM_PATTERN)?.[1]?.trim() ?? null);
  if (items.some((item) => item == null)) {
    return false;
  }
  const lengths = items.map((item) => item!.length);
  const averageLength = lengths.reduce((sum, length) => sum + length, 0) / lengths.length;
  const maxLength = Math.max(...lengths);
  return averageLength <= 12 && maxLength <= 24;
}

function normalizeShortChatParagraphBreaks(text: string): string {
  const paragraphs = text
    .split(/\n{2,}/)
    .map((paragraph) => paragraph.trim())
    .filter((paragraph) => paragraph.length > 0);
  if (paragraphs.length < 2) {
    return text;
  }
  if (paragraphs.some((paragraph) => paragraph.includes("\n"))) {
    return text;
  }
  const lengths = paragraphs.map((paragraph) => paragraph.length);
  const averageLength = lengths.reduce((sum, length) => sum + length, 0) / lengths.length;
  const maxLength = Math.max(...lengths);
  if (averageLength > 18 || maxLength > 40) {
    return text;
  }
  return paragraphs.join("\n");
}

function shouldPreferPlainTextTeamChat(text: string): boolean {
  if (!text.includes("\n")) {
    return false;
  }
  if (hasExplicitStructuredMarkdown(text)) {
    return false;
  }
  return normalizeShortChatParagraphBreaks(text) !== text || isShortListLikeChat(text);
}

export function renderMarkdownWithMentions(
  text: string,
  displayNameByActorId?: Record<string, string>
): string {
  if (shouldPreferPlainTextTeamChat(text)) {
    return renderCanonicalPlainTextWithMentions(
      normalizeShortChatParagraphBreaks(text),
      displayNameByActorId
    );
  }
  const tokenized = replaceCanonicalMentionsWithTokens(
    replaceRawMentionsOutsideMarkdownProtectedRanges(text)
  );
  const rendered = renderTeamMarkdownCached(tokenized);
  return renderMentionTokensIntoHtml(rendered, displayNameByActorId);
}

export function renderPlainTextWithMentions(
  text: string,
  displayNameByActorId?: Record<string, string>
): string {
  const tokenized = tokenizePlainTextMentions(text);
  const escaped = escapeTeamHtml(tokenized).replace(/\n/g, "<br/>");
  return renderMentionTokensIntoHtml(escaped, displayNameByActorId);
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
    case "coordinator_task_assignment":
      return {
        type: "coordinator_task_assignment",
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
        description: "Planner, reviewer, and runtime owner for database changes.",
      };
    default:
      return {};
  }
}
