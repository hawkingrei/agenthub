import { HoverCard } from "@mantine/core";
import React from "react";
import {
  type AcpPermissionOption,
  type AcpPermissionRecord,
  TeamConversationMessageRecord,
  api,
} from "../api";
import { preloadThreadMarkdownAssets, ThreadRichText } from "../components/thread_rich_text";
import {
  DEFAULT_CONVERSATION_TAIL_WINDOW_SIZE,
  windowConversation,
} from "../conversation";
import { deriveThreadJumpState, deriveThreadStickToBottom } from "../hooks/thread_viewport";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  createDisplayNameLookup,
  isHumanMailboxActor,
  parseStructuredTeamPayload,
  type MentionCandidate,
  renderMarkdownWithMentions,
  resolveMentionDraftQuery,
  resolveDisplayName,
  resolveVisibleTeamPayloadText,
  type MentionDraftQuery,
} from "./team/mailbox_helpers";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
} from "../ui/tailwind_classes";

type TeamTaskPanelProps = {
  developerMode: boolean;
  token?: string | null;
  tasksLoading?: boolean;
  onRefreshTasks?: () => Promise<void> | void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: (payload: { text: string; mentionActorIds: string[] }) => Promise<void> | void;
  onRefreshMessages?: () => Promise<void> | void;
  onLoadOlderMessages?: () => Promise<void> | void;
  canLoadOlderMessages?: boolean;
  loadingOlderMessages?: boolean;
  messages: TeamConversationMessageRecord[];
  seenByMessageId?: Record<number, string[]>;
  humanActorId?: string;
  memberLiveStates?: TeamMemberLiveState[];
  memberIds?: string[];
  messagesLoading: boolean;
  busy: string | null;
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

type PermissionReviewCardPayload = {
  type: "permission_review_card";
  permission_id: string;
  agent_id: string;
  agent_session_id?: string | null;
  acp_session_id?: string | null;
  tool_call_id?: string | null;
  tool_call?: unknown;
  tool_name?: string | null;
  requester_actor_id?: string | null;
  requester_role?: string | null;
  options: AcpPermissionOption[];
  summary?: string | null;
  reason?: string | null;
  reason_text?: string | null;
  status?: string | null;
};

type PermissionToneAudioContextConstructor = new () => PermissionToneAudioContext;

type PermissionToneAudioContext = {
  currentTime: number;
  destination: unknown;
  state?: string;
  createOscillator: () => PermissionToneOscillator;
  createGain: () => PermissionToneGainNode;
  resume?: () => Promise<void>;
  close?: () => Promise<void>;
};

type PermissionToneOscillator = {
  type: string;
  frequency: {
    setValueAtTime: (value: number, time: number) => void;
    linearRampToValueAtTime: (value: number, time: number) => void;
  };
  connect: (target: PermissionToneGainNode) => void;
  start: () => void;
  stop: (when?: number) => void;
  onended: (() => void) | null;
};

type PermissionToneGainNode = {
  gain: {
    setValueAtTime: (value: number, time: number) => void;
    exponentialRampToValueAtTime: (value: number, time: number) => void;
  };
  connect: (target: unknown) => void;
};

type TeamTaskPanelAudioWindow = Window &
  typeof globalThis & {
    AudioContext?: PermissionToneAudioContextConstructor;
    webkitAudioContext?: PermissionToneAudioContextConstructor;
  };

const TEAM_TASK_COMPOSER_PANEL_CLASS =
  "flex shrink-0 flex-col gap-2 border-t border-black/[0.05] bg-white/90 px-3 py-2.5 shadow-[0_-1px_0_rgba(15,23,42,0.02)]";
const TEAM_TASK_SHORTCUT_CLASS = "text-ui-xs text-ui-text-muted";
const TEAM_TASK_COMPOSER_META_ROW_CLASS =
  "flex flex-wrap items-center justify-between gap-2";
const TEAM_TASK_MESSAGE_EMPTY_CLASS =
  "px-1 py-2 text-ui-sm text-ui-text-muted";
const TEAM_TASK_ACTIVITY_LIST_CLASS =
  "min-h-0 flex-1 overflow-y-auto pr-0.5";
const TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS =
  "min-h-0 flex-1 overflow-y-auto pr-0.5";
const TEAM_TASK_ACTIVITY_SHELL_CLASS =
  "rounded-[14px] border border-black/[0.05] bg-white/88 px-2.5 py-2 shadow-[0_1px_3px_rgba(15,23,42,0.04)] sm:px-3 sm:py-2.5";
const TEAM_TASK_ACTIVITY_STACK_CLASS =
  "flex w-full flex-col gap-1.5";
const TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS =
  "acp-bubble relative rounded-[13px] border px-2.5 py-2 shadow-[0_1px_2px_rgba(15,23,42,0.03)] sm:px-3 sm:py-2.5";
const TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS =
  `${TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS} border-[rgba(59,130,246,0.14)] bg-[rgba(244,248,255,0.94)] text-ui-text-primary`;
const TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS =
  `${TEAM_TASK_ACTIVITY_ITEM_BASE_CLASS} border-black/[0.06] bg-white/94 text-ui-text-primary`;
const TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS =
  "flex items-start justify-between gap-2";
const TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS =
  "flex min-w-0 flex-wrap items-center gap-1.5";
const TEAM_TASK_ACTIVITY_AUTHOR_CLASS =
  "text-[13px] font-semibold tracking-tight text-ui-text-primary";
const TEAM_TASK_ACTIVITY_TIME_CLASS =
  "text-[10px] font-medium uppercase tracking-[0.12em] text-ui-text-muted/90";
const TEAM_TASK_ACTIVITY_BODY_CLASS =
  "mt-1.5 min-w-0 break-words text-[12.5px] leading-5 text-ui-text-primary sm:text-[13px] sm:leading-6";
const TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS =
  "mono mt-1.5 max-w-full overflow-x-auto whitespace-pre rounded-[10px] border border-black/[0.05] bg-black/[0.028] px-2 py-1.5 text-[10px] leading-[1.35] text-ui-text-secondary sm:px-2.5 sm:py-2 sm:text-[10.5px]";
const TEAM_TASK_PERMISSION_CARD_CLASS =
  "mt-1 rounded-[12px] border border-black/[0.06] bg-[rgba(252,250,245,0.94)] px-2.5 py-2.5 sm:px-3 sm:py-3";
const TEAM_TASK_PERMISSION_CARD_COMPACT_CLASS =
  "mt-1 rounded-[11px] border border-black/[0.06] bg-[rgba(252,251,247,0.88)] px-2.5 py-1.5";
const TEAM_TASK_PERMISSION_CARD_HEADER_CLASS =
  "flex flex-wrap items-center justify-between gap-2";
const TEAM_TASK_PERMISSION_CARD_TITLE_CLASS =
  "text-[13px] font-semibold tracking-tight text-ui-text-primary";
const TEAM_TASK_PERMISSION_CARD_STATUS_CLASS =
  "inline-flex items-center rounded-full border border-black/[0.06] bg-white/[0.84] px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_PERMISSION_CARD_COMPACT_PREVIEW_CLASS =
  "mono mt-1 max-w-full overflow-hidden text-ellipsis whitespace-nowrap text-[10px] text-ui-text-muted";
const TEAM_TASK_PERMISSION_CARD_BODY_CLASS =
  "mt-2 space-y-2 text-[13px] leading-6 text-ui-text-secondary";
const TEAM_TASK_PERMISSION_CARD_REASON_CLASS =
  "text-[11px] font-medium uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_PERMISSION_CARD_ACTIONS_CLASS =
  "mt-3 flex flex-wrap items-center gap-2";
const TEAM_TASK_PERMISSION_CARD_SECONDARY_BUTTON_CLASS =
  "inline-flex items-center rounded-full border border-black/[0.06] bg-white/[0.86] px-2.5 py-1 text-[11px] font-medium text-ui-text-muted transition hover:border-black/[0.1] hover:bg-black/[0.03] hover:text-ui-text-primary disabled:cursor-not-allowed disabled:opacity-60";
const TEAM_TASK_PERMISSION_CARD_ERROR_CLASS = "text-xs text-red-600";
const TEAM_TASK_ACTIVITY_DETAILS_CLASS =
  "mt-2 rounded-[11px] border border-black/[0.05] bg-black/[0.018]";
const TEAM_TASK_ACTIVITY_DETAILS_BUTTON_CLASS =
  "mt-1.5 inline-flex items-center rounded-full border border-black/[0.06] bg-white/[0.88] px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted transition hover:border-black/[0.1] hover:bg-black/[0.03]";
const TEAM_TASK_ACTIVITY_DETAILS_GRID_CLASS =
  "grid gap-1.5 border-t border-black/[0.05] px-2.5 py-2 text-[11px] text-ui-text-muted sm:grid-cols-2";
const TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS =
  "mono font-medium text-ui-text-secondary";
const TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS =
  "inline-flex h-5 min-w-5 items-center justify-center rounded-full border border-black/[0.06] bg-white/[0.88] p-0.5 text-[10px] font-medium text-ui-text-muted transition hover:border-black/[0.1] hover:bg-black/[0.03] hover:text-ui-text-primary";
const TEAM_TASK_ACTIVITY_SEEN_META_CLASS = "absolute bottom-1.5 right-1.5 z-[1]";
const TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS =
  "mt-2 flex flex-wrap items-center gap-2 text-xs text-ui-text-muted";
const TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS =
  "inline-flex h-3 w-3 rounded-full border border-black/10 bg-[rgba(55,53,47,0.18)]";
const TEAM_TASK_ACTIVITY_SEEN_DIAL_CLASS =
  "relative inline-flex items-center justify-center overflow-hidden rounded-full align-middle shadow-[inset_0_0_0_1px_rgba(0,0,0,0.06)]";
const TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS =
  "min-w-[220px] rounded-[12px] border border-black/[0.06] bg-[rgba(252,251,247,0.96)] p-3 shadow-[0_4px_12px_rgba(15,23,42,0.06)]";
const TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS =
  "text-[11px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS =
  "mt-1 text-sm font-semibold tracking-tight text-ui-text-primary";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS = "mt-3";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS =
  "text-[10px] font-semibold uppercase tracking-[0.12em] text-ui-text-muted";
const TEAM_TASK_JUMP_BUTTON_CLASS =
  "inline-flex h-7 w-7 items-center justify-center rounded-full border border-black/[0.08] bg-white/[0.9] text-ui-text-secondary shadow-[0_2px_6px_rgba(15,23,42,0.06)] backdrop-blur transition hover:border-black/[0.1] hover:text-ui-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ui-border-strong sm:h-8 sm:w-8";
const TEAM_TASK_TAIL_WINDOW_SIZE = DEFAULT_CONVERSATION_TAIL_WINDOW_SIZE;
const TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT = 116;

function getPermissionToneAudioContextConstructor(): PermissionToneAudioContextConstructor | null {
  if (typeof window === "undefined") {
    return null;
  }
  const toneWindow = window as TeamTaskPanelAudioWindow;
  return toneWindow.AudioContext ?? toneWindow.webkitAudioContext ?? null;
}

function closePermissionToneAudioContext(context: PermissionToneAudioContext | null): void {
  if (!context?.close) {
    return;
  }
  void context.close().catch(() => {});
}

async function playHumanReviewFallbackTone(): Promise<void> {
  const AudioContextCtor = getPermissionToneAudioContextConstructor();
  if (!AudioContextCtor) {
    return;
  }
  let context: PermissionToneAudioContext | null = null;
  try {
    context = new AudioContextCtor();
    const oscillator = context.createOscillator();
    const gainNode = context.createGain();
    oscillator.type = "triangle";
    oscillator.frequency.setValueAtTime(880, context.currentTime);
    oscillator.frequency.linearRampToValueAtTime(660, context.currentTime + 0.16);
    gainNode.gain.setValueAtTime(0.0001, context.currentTime);
    gainNode.gain.exponentialRampToValueAtTime(0.08, context.currentTime + 0.01);
    gainNode.gain.exponentialRampToValueAtTime(0.0001, context.currentTime + 0.24);
    oscillator.connect(gainNode);
    gainNode.connect(context.destination);
    oscillator.onended = () => {
      closePermissionToneAudioContext(context);
    };
    if (context.state === "suspended" && context.resume) {
      await context.resume();
    }
    oscillator.start();
    oscillator.stop(context.currentTime + 0.26);
  } catch {
    if (context) {
      closePermissionToneAudioContext(context);
    }
  }
}

function resolveMessageText(
  message: TeamConversationMessageRecord,
  toPrettyJson: (value: unknown) => string
): string {
  const visibleText = resolveVisibleTeamPayloadText(message.payload);
  if (visibleText !== null) {
    return visibleText;
  }
  return toPrettyJson(message.payload);
}

function resolveThreadAuthorLabel(
  actorId: string,
  humanActorId: string,
  liveStateByMemberId: Map<string, TeamMemberLiveState>
): string {
  if (isHumanMailboxActor(actorId, humanActorId)) {
    return "You";
  }
  const state = liveStateByMemberId.get(actorId);
  const agentName = state?.agent_name?.trim();
  if (agentName) {
    return agentName;
  }
  return actorId;
}

function resolveMentionLabel(
  actorId: string,
  liveStateByMemberId: Map<string, TeamMemberLiveState>
): string {
  const state = liveStateByMemberId.get(actorId);
  const agentName = state?.agent_name?.trim();
  if (agentName) {
    return agentName;
  }
  return actorId;
}

function resolveActivityItemClassName(
  actorId: string,
  humanActorId: string
): string {
  return isHumanMailboxActor(actorId, humanActorId)
    ? TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS
    : TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS;
}

function normalizeTrimmedString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

function normalizePermissionOption(value: unknown): AcpPermissionOption | null {
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const record = value as Record<string, unknown>;
  const optionId = normalizeTrimmedString(record.option_id);
  const name = normalizeTrimmedString(record.name);
  if (!optionId || !name) {
    return null;
  }
  return {
    option_id: optionId,
    name,
    kind: typeof record.kind === "string" ? record.kind.trim() : "",
  };
}

function isCompactCommandLikeText(text: string): boolean {
  const normalized = text.trim();
  if (normalized.length === 0) {
    return false;
  }
  return (
    normalized.startsWith("Run ") ||
    normalized.startsWith("$ ") ||
    normalized.startsWith("/bin/zsh ") ||
    normalized.startsWith("/bin/bash ")
  );
}

function normalizePermissionOptions(value: unknown): AcpPermissionOption[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.flatMap((candidate) => {
    const option = normalizePermissionOption(candidate);
    return option ? [option] : [];
  });
}

function normalizePermissionRecord(record: AcpPermissionRecord): AcpPermissionRecord {
  const raw = record as Record<string, unknown>;
  return {
    ...record,
    options: normalizePermissionOptions(raw.options),
    selected_option_id: normalizeTrimmedString(raw.selected_option_id),
    status: typeof raw.status === "string" ? raw.status : String(raw.status ?? ""),
  };
}

function equalPermissionOptions(
  left: AcpPermissionOption[],
  right: AcpPermissionOption[]
): boolean {
  if (left.length !== right.length) {
    return false;
  }
  return left.every((option, index) => {
    const candidate = right[index];
    return (
      candidate?.option_id === option.option_id &&
      candidate.name === option.name &&
      candidate.kind === option.kind
    );
  });
}

function equalPermissionRecords(
  left: AcpPermissionRecord,
  right: AcpPermissionRecord
): boolean {
  return (
    left.id === right.id &&
    left.agent_id === right.agent_id &&
    left.session_id === right.session_id &&
    (left.acp_session_id ?? null) === (right.acp_session_id ?? null) &&
    (left.tool_call_id ?? null) === (right.tool_call_id ?? null) &&
    left.status === right.status &&
    (left.selected_option_id ?? null) === (right.selected_option_id ?? null) &&
    left.created_at === right.created_at &&
    (left.responded_at ?? null) === (right.responded_at ?? null) &&
    equalPermissionOptions(left.options, right.options)
  );
}

function parsePermissionReviewCardPayload(payload: unknown): PermissionReviewCardPayload | null {
  const parsed = parseStructuredTeamPayload(payload);
  if (typeof parsed !== "object" || parsed === null) {
    return null;
  }
  const value = parsed as Record<string, unknown>;
  if (value.type !== "permission_review_card") {
    return null;
  }
  const permissionId = String(value.permission_id ?? "").trim();
  const agentId = String(value.agent_id ?? "").trim();
  if (!permissionId || !agentId) {
    return null;
  }
  const options = normalizePermissionOptions(value.options);
  return {
    type: "permission_review_card",
    permission_id: permissionId,
    agent_id: agentId,
    agent_session_id:
      typeof value.agent_session_id === "string" ? value.agent_session_id : null,
    acp_session_id:
      typeof value.acp_session_id === "string" ? value.acp_session_id : null,
    tool_call_id: typeof value.tool_call_id === "string" ? value.tool_call_id : null,
    tool_call: value.tool_call,
    tool_name: typeof value.tool_name === "string" ? value.tool_name : null,
    requester_actor_id:
      typeof value.requester_actor_id === "string" ? value.requester_actor_id : null,
    requester_role:
      typeof value.requester_role === "string" ? value.requester_role : null,
    options,
    summary: typeof value.summary === "string" ? value.summary : null,
    reason: typeof value.reason === "string" ? value.reason : null,
    reason_text: typeof value.reason_text === "string" ? value.reason_text : null,
    status: typeof value.status === "string" ? value.status : null,
  };
}

function buildPermissionRecordStub(
  payload: PermissionReviewCardPayload,
  optionId?: string
): AcpPermissionRecord {
  return {
    id: payload.permission_id,
    agent_id: payload.agent_id,
    session_id: payload.agent_session_id ?? "",
    acp_session_id: payload.acp_session_id ?? null,
    tool_call_id: payload.tool_call_id ?? null,
    options: payload.options,
    tool_call: payload.tool_call ?? null,
    status: "responded",
    selected_option_id: optionId ?? null,
    created_at: Math.floor(Date.now() / 1000),
    responded_at: Math.floor(Date.now() / 1000),
  };
}

function resolvePermissionCardStatus(
  payload: PermissionReviewCardPayload,
  record?: AcpPermissionRecord
): string {
  const recordStatus = normalizeTrimmedString(record?.status);
  if (recordStatus) {
    return recordStatus;
  }
  const reason = normalizeTrimmedString(payload.reason)?.toLowerCase();
  if (reason === "review_timeout" || reason === "timed_out" || reason === "timeout") {
    return "timeout";
  }
  return normalizeTrimmedString(payload.status) ?? "pending";
}

function resolvePermissionStatusText(
  payload: PermissionReviewCardPayload,
  record?: AcpPermissionRecord
): string {
  const status = resolvePermissionCardStatus(payload, record);
  if (status === "responded") {
    const normalizedRecord = record ?? buildPermissionRecordStub(payload);
    const selectedOptionId = normalizeTrimmedString(normalizedRecord.selected_option_id);
    if (!selectedOptionId) {
      return "Cancelled";
    }
    const option = normalizedRecord.options.find(
      (candidate) => candidate.option_id === selectedOptionId
    );
    return option ? `Approved · ${option.name}` : "Approved";
  }
  if (status === "timeout") {
    return "Timed out";
  }
  if (status === "pending") {
    return "Awaiting human review";
  }
  return status;
}

function resolvePermissionToolPreview(payload: PermissionReviewCardPayload): string | null {
  const preview = normalizeTrimmedString(payload.tool_name) ?? normalizeTrimmedString(payload.summary);
  if (!preview) {
    return null;
  }
  return preview
    .split(/\r?\n/, 1)[0]
    ?.replace(/\s+/g, " ")
    .trim() || null;
}

function resolvePermissionToolLabel(payload: PermissionReviewCardPayload): string {
  const preview = resolvePermissionToolPreview(payload);
  if (!preview) {
    return "Permission review";
  }
  const normalized = preview.toLowerCase();
  if (preview.length > 72 || normalized.startsWith("run ") || normalized.startsWith("/bin/")) {
    return "Command review";
  }
  return preview;
}

type SeenProgressState = {
  readActorIds: string[];
  unreadActorIds: string[];
  totalCount: number;
  readCount: number;
  unreadCount: number;
  progress: number;
};

type SeenDialStyle = React.CSSProperties & {
  "--value": number;
  "--size": string;
  "--thickness": string;
};

type PermissionReviewCardProps = {
  payload: PermissionReviewCardPayload;
  permissionRecord?: AcpPermissionRecord;
  busy: boolean;
  errorText?: string;
  onRespond: (payload: PermissionReviewCardPayload, optionId?: string) => void;
};

function PermissionReviewCard(props: PermissionReviewCardProps) {
  const { payload, permissionRecord, busy, errorText, onRespond } = props;
  const status = resolvePermissionCardStatus(payload, permissionRecord);
  const statusText = resolvePermissionStatusText(payload, permissionRecord);
  const isPending = status === "pending";
  const toolLabel = resolvePermissionToolLabel(payload);
  const toolPreview = resolvePermissionToolPreview(payload);
  const cardClassName = isPending
    ? TEAM_TASK_PERMISSION_CARD_CLASS
    : TEAM_TASK_PERMISSION_CARD_COMPACT_CLASS;

  return (
    <div className={cardClassName} data-team-permission-card="true">
      <div className={TEAM_TASK_PERMISSION_CARD_HEADER_CLASS}>
        <span className={TEAM_TASK_PERMISSION_CARD_TITLE_CLASS}>{toolLabel}</span>
        <span className={TEAM_TASK_PERMISSION_CARD_STATUS_CLASS}>{statusText}</span>
      </div>
      {!isPending && toolPreview && toolPreview !== toolLabel && (
        <div className={TEAM_TASK_PERMISSION_CARD_COMPACT_PREVIEW_CLASS}>{toolPreview}</div>
      )}
      {isPending ? (
        <div className={TEAM_TASK_PERMISSION_CARD_BODY_CLASS}>
          {payload.reason_text && (
            <div className={TEAM_TASK_PERMISSION_CARD_REASON_CLASS}>{payload.reason_text}</div>
          )}
          <>
            {(payload.summary ?? toolPreview) && <div>{payload.summary ?? toolPreview}</div>}
            <div className={TEAM_TASK_PERMISSION_CARD_ACTIONS_CLASS}>
              {payload.options.map((option, index) => {
                const optionId = option.option_id.trim();
                return (
                  <button
                    key={`${optionId}:${index}`}
                    type="button"
                    className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
                    disabled={busy || !optionId}
                    onClick={() => onRespond(payload, optionId)}
                  >
                    {option.name}
                  </button>
                );
              })}
              <button
                type="button"
                className={TEAM_TASK_PERMISSION_CARD_SECONDARY_BUTTON_CLASS}
                disabled={busy}
                onClick={() => onRespond(payload)}
              >
                Cancel
              </button>
            </div>
            {errorText ? <div className={TEAM_TASK_PERMISSION_CARD_ERROR_CLASS}>{errorText}</div> : null}
          </>
        </div>
      ) : null}
    </div>
  );
}

function resolveSeenProgressState(
  seenActorIds: string[],
  memberIds: string[],
  authorActorId: string
): SeenProgressState {
  const normalizedReadActorIds: string[] = [];
  const seen = new Set<string>();
  for (const actorId of seenActorIds) {
    const normalized = actorId.trim();
    if (!normalized || normalized === authorActorId || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    normalizedReadActorIds.push(normalized);
  }

  const recipientActorIds: string[] = [];
  const recipientSet = new Set<string>();
  for (const memberId of memberIds) {
    const normalized = memberId.trim();
    if (!normalized || normalized === authorActorId || recipientSet.has(normalized)) {
      continue;
    }
    recipientSet.add(normalized);
    recipientActorIds.push(normalized);
  }
  for (const actorId of normalizedReadActorIds) {
    if (recipientSet.has(actorId)) {
      continue;
    }
    recipientSet.add(actorId);
    recipientActorIds.push(actorId);
  }

  const unreadActorIds = recipientActorIds.filter((actorId) => !seen.has(actorId));
  const totalCount = recipientActorIds.length;
  const readCount = normalizedReadActorIds.length;
  return {
    readActorIds: normalizedReadActorIds,
    unreadActorIds,
    totalCount,
    readCount,
    unreadCount: unreadActorIds.length,
    progress: totalCount > 0 ? Math.round((readCount / totalCount) * 100) : 0,
  };
}

function TeamTaskPanelImpl(props: TeamTaskPanelProps) {
  const {
    developerMode,
    token = null,
    messageDraft,
    onMessageDraftChange,
    onLoadOlderMessages,
    onRefreshMessages,
    onSendMessage,
    messages,
    seenByMessageId = {},
    humanActorId = "user",
    memberLiveStates = [],
    memberIds = [],
    messagesLoading,
    canLoadOlderMessages = false,
    loadingOlderMessages = false,
    busy,
    formatTs,
    toPrettyJson,
  } = props;

  React.useEffect(() => {
    void preloadThreadMarkdownAssets().catch(() => {});
  }, []);
  const messageTextareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const [activeMention, setActiveMention] = React.useState<MentionDraftQuery | null>(null);
  const [activeMentionIndex, setActiveMentionIndex] = React.useState(0);
  const [expandedItemKeys, setExpandedItemKeys] = React.useState<Record<string, boolean>>({});
  const [permissionRecordsById, setPermissionRecordsById] = React.useState<
    Record<string, AcpPermissionRecord>
  >({});
  const [permissionBusyId, setPermissionBusyId] = React.useState<string | null>(null);
  const [permissionErrorById, setPermissionErrorById] = React.useState<Record<string, string>>({});
  const activityListRef = React.useRef<HTMLDivElement | null>(null);
  const lastActivityScrollTopRef = React.useRef<number | null>(null);
  const [stickToBottom, setStickToBottom] = React.useState(true);
  const [historyExpanded, setHistoryExpanded] = React.useState(false);
  const conversationId = messages[0]?.conversation_id ?? "";
  React.useEffect(() => {
    setHistoryExpanded(false);
  }, [conversationId]);
  const liveStateByMemberId = React.useMemo(
    () => new Map(memberLiveStates.map((member) => [member.member_id, member])),
    [memberLiveStates]
  );
  const memberDisplayNamesById = React.useMemo(
    () =>
      createDisplayNameLookup(
        memberIds.map((memberId) => [memberId, resolveMentionLabel(memberId, liveStateByMemberId)])
      ),
    [liveStateByMemberId, memberIds]
  );

  const mentionCandidates = React.useMemo<MentionCandidate[]>(() => {
    const seen = new Set<string>();
    const items: MentionCandidate[] = [];
    for (const memberId of [...memberIds, ...memberLiveStates.map((member) => member.member_id)]) {
      const normalized = memberId.trim();
      if (!normalized || seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      items.push({
        actorId: normalized,
        label: resolveMentionLabel(normalized, liveStateByMemberId),
        aliases: [normalized],
      });
    }
    return items;
  }, [liveStateByMemberId, memberIds, memberLiveStates]);
  const filteredMentionCandidates = React.useMemo(() => {
    if (!activeMention) {
      return [];
    }
    const keyword = activeMention.keyword.trim().toLowerCase();
    return mentionCandidates
      .filter((candidate) =>
        keyword.length === 0
          ? true
          : [candidate.label, candidate.actorId, ...candidate.aliases].some((value) =>
              value.toLowerCase().startsWith(keyword)
            )
      )
      .slice(0, 8);
  }, [activeMention, mentionCandidates]);

  const updateMentionQuery = React.useCallback((draft: string, cursor: number | null) => {
    if (cursor === null || Number.isNaN(cursor)) {
      setActiveMention(null);
      setActiveMentionIndex(0);
      return;
    }
    const next = resolveMentionDraftQuery(draft, cursor);
    setActiveMention(next);
    setActiveMentionIndex(0);
  }, []);

  const applyMentionSelection = React.useCallback(
    (candidate: MentionCandidate) => {
      if (!activeMention) {
        return;
      }
      const applied = applyMentionAtTag(messageDraft, activeMention, candidate.label);
      onMessageDraftChange(applied.text);
      setActiveMention(null);
      setActiveMentionIndex(0);
      requestAnimationFrame(() => {
        const textarea = messageTextareaRef.current;
        if (!textarea) {
          return;
        }
        textarea.focus();
        textarea.setSelectionRange(applied.cursor, applied.cursor);
      });
    },
    [activeMention, messageDraft, onMessageDraftChange]
  );

  const canSendMessage = messageDraft.trim().length > 0 && busy !== "send-task-message";
  const sendCurrentMessage = React.useCallback(() => {
    const normalizedDraft = canonicalizeMentionDraft(messageDraft, mentionCandidates);
    if (!normalizedDraft.text.trim()) {
      return;
    }
    void onSendMessage({
      text: normalizedDraft.text,
      mentionActorIds: normalizedDraft.mentionActorIds,
    });
  }, [mentionCandidates, messageDraft, onSendMessage]);
  const orderedMessages = React.useMemo(
    () =>
      [...messages].sort((left, right) => {
        if (left.created_at !== right.created_at) {
          return left.created_at - right.created_at;
        }
        if (left.message_id !== right.message_id) {
          return left.message_id - right.message_id;
        }
        return left.from_actor_id.localeCompare(right.from_actor_id);
      }),
    [messages]
  );
  const permissionCardTargets = React.useMemo(
    () =>
      orderedMessages.flatMap((message) => {
        const payload = parsePermissionReviewCardPayload(message.payload);
        return payload
          ? [{ permissionId: payload.permission_id, agentId: payload.agent_id }]
          : [];
      }),
    [orderedMessages]
  );
  const permissionCardTargetKey = React.useMemo(
    () =>
      permissionCardTargets
        .map((target) => `${target.agentId}:${target.permissionId}`)
        .sort()
        .join("|"),
    [permissionCardTargets]
  );
  const humanReviewToneInitializedRef = React.useRef(false);
  const humanReviewToneSeenPermissionIdsRef = React.useRef<Set<string>>(new Set());
  const pendingHumanReviewPermissionIds = React.useMemo(() => {
    const seen = new Set<string>();
    const ids: string[] = [];
    for (const message of orderedMessages) {
      const payload = parsePermissionReviewCardPayload(message.payload);
      if (!payload) {
        continue;
      }
      const status = permissionRecordsById[payload.permission_id]?.status ?? payload.status ?? "pending";
      if (status !== "pending" || seen.has(payload.permission_id)) {
        continue;
      }
      seen.add(payload.permission_id);
      ids.push(payload.permission_id);
    }
    return ids;
  }, [orderedMessages, permissionRecordsById]);
  const refreshPermissionCards = React.useCallback(async () => {
    if (!token || permissionCardTargets.length === 0) {
      return;
    }
    const permissionIds = new Set(permissionCardTargets.map((target) => target.permissionId));
    const agentIds = [...new Set(permissionCardTargets.map((target) => target.agentId))];
    const results = await Promise.allSettled(
      agentIds.map(async (agentId) => ({
        agentId,
        items: await api.listAcpPermissions(token, agentId),
      }))
    );
    const nextRecords: Record<string, AcpPermissionRecord> = {};
    for (const result of results) {
      if (result.status !== "fulfilled") {
        continue;
      }
      for (const item of result.value.items) {
        if (!permissionIds.has(item.id)) {
          continue;
        }
        nextRecords[item.id] = normalizePermissionRecord(item);
      }
    }
    setPermissionRecordsById((current) => {
      let next: Record<string, AcpPermissionRecord> | null = null;
      for (const target of permissionCardTargets) {
        const incoming = nextRecords[target.permissionId];
        if (!incoming) {
          continue;
        }
        const existing = current[target.permissionId];
        if (existing?.status === "responded" && incoming.status === "pending") {
          continue;
        }
        if (existing && equalPermissionRecords(existing, incoming)) {
          continue;
        }
        if (next === null) {
          next = { ...current };
        }
        next[target.permissionId] = incoming;
      }
      return next ?? current;
    });
  }, [permissionCardTargets, token]);
  React.useEffect(() => {
    if (!humanReviewToneInitializedRef.current) {
      humanReviewToneSeenPermissionIdsRef.current = new Set(pendingHumanReviewPermissionIds);
      humanReviewToneInitializedRef.current = true;
      return;
    }
    let shouldPlayTone = false;
    for (const permissionId of pendingHumanReviewPermissionIds) {
      if (humanReviewToneSeenPermissionIdsRef.current.has(permissionId)) {
        continue;
      }
      humanReviewToneSeenPermissionIdsRef.current.add(permissionId);
      shouldPlayTone = true;
    }
    if (shouldPlayTone) {
      void playHumanReviewFallbackTone();
    }
  }, [pendingHumanReviewPermissionIds]);
  const activityWindow = React.useMemo(
    () =>
      historyExpanded
        ? { items: orderedMessages, offset: 0, total: orderedMessages.length }
        : windowConversation(orderedMessages, stickToBottom, TEAM_TASK_TAIL_WINDOW_SIZE),
    [historyExpanded, orderedMessages, stickToBottom]
  );
  const visibleWaterfallItems = React.useMemo(
    () =>
      activityWindow.items.map((message) => {
        const permissionCardPayload = parsePermissionReviewCardPayload(message.payload);
        return {
          key: `conversation-${message.message_id}`,
          sequence: message.message_id,
          createdAt: message.created_at,
          fromActorId: message.from_actor_id,
          toActorId: message.to_actor_id ?? null,
          routeOrStatus: message.route,
          streamLabel: "conversation",
          payload: message.payload,
          text: permissionCardPayload ? "" : resolveMessageText(message, toPrettyJson),
        };
      }),
    [activityWindow.items, toPrettyJson]
  );
  const hiddenWaterfallCount = activityWindow.offset;
  const hiddenWaterfallSpacerHeight = React.useMemo(() => {
    if (!stickToBottom || hiddenWaterfallCount <= 0) {
      return 0;
    }
    return hiddenWaterfallCount * TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT;
  }, [hiddenWaterfallCount, stickToBottom]);
  const activityListClassName =
    messagesLoading || orderedMessages.length > 0
      ? TEAM_TASK_ACTIVITY_LIST_CLASS
      : TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS;
  const latestWaterfallKey =
    orderedMessages.length > 0
      ? `conversation-${orderedMessages[orderedMessages.length - 1]?.message_id ?? "empty"}`
      : "empty";
  const activityJumpState = React.useMemo(
    () =>
      deriveThreadJumpState({
        active: orderedMessages.length > 0,
        stickToBottom,
        pendingCount: 0,
      }),
    [orderedMessages.length, stickToBottom]
  );
  const renderTeamMessageHtml = React.useCallback(
    (text: string) => renderMarkdownWithMentions(text, memberDisplayNamesById),
    [memberDisplayNamesById]
  );
  const onRespondPermission = React.useCallback(
    async (payload: PermissionReviewCardPayload, optionId?: string) => {
      if (!token) {
        return;
      }
      setPermissionBusyId(payload.permission_id);
      setPermissionErrorById((current) => {
        if (!(payload.permission_id in current)) {
          return current;
        }
        const next = { ...current };
        delete next[payload.permission_id];
        return next;
      });
      try {
        await api.respondAcpPermission(token, payload.agent_id, payload.permission_id, {
          option_id: optionId ?? null,
          outcome: optionId ? undefined : "cancelled",
        });
        setPermissionRecordsById((current) => ({
          ...current,
          [payload.permission_id]: {
            ...(current[payload.permission_id] ?? buildPermissionRecordStub(payload, optionId)),
            status: "responded",
            selected_option_id: optionId ?? null,
            responded_at: Math.floor(Date.now() / 1000),
          },
        }));
      } catch (error) {
        const message =
          error instanceof Error && error.message.trim().length > 0
            ? error.message.trim()
            : "Failed to respond to permission request";
        setPermissionErrorById((current) => ({
          ...current,
          [payload.permission_id]: message,
        }));
      } finally {
        setPermissionBusyId(null);
      }
    },
    [token]
  );

  const scrollActivityToBottom = React.useCallback(() => {
    const node = activityListRef.current;
    if (!node) {
      return;
    }
    node.scrollTop = node.scrollHeight;
    lastActivityScrollTopRef.current = node.scrollTop;
  }, []);

  React.useEffect(() => {
    if (!token || permissionCardTargets.length === 0) {
      return;
    }
    void refreshPermissionCards();
  }, [permissionCardTargetKey, permissionCardTargets.length, refreshPermissionCards, token]);

  React.useEffect(() => {
    if (!token || permissionCardTargets.length === 0) {
      return;
    }
    const hasPendingPermission = permissionCardTargets.some((target) => {
      const record = permissionRecordsById[target.permissionId];
      return !record || record.status === "pending";
    });
    if (!hasPendingPermission) {
      return;
    }
    const intervalId = window.setInterval(() => {
      void refreshPermissionCards();
    }, 5000);
    return () => {
      window.clearInterval(intervalId);
    };
  }, [
    permissionCardTargets,
    permissionRecordsById,
    refreshPermissionCards,
    token,
  ]);

  React.useEffect(() => {
    if (messagesLoading || orderedMessages.length === 0 || !stickToBottom) {
      return;
    }
    const handle = window.requestAnimationFrame(() => {
      scrollActivityToBottom();
    });
    return () => {
      window.cancelAnimationFrame(handle);
    };
  }, [latestWaterfallKey, messagesLoading, orderedMessages.length, scrollActivityToBottom, stickToBottom]);

  const handleActivityScroll = React.useCallback(() => {
    const node = activityListRef.current;
    if (!node) {
      return;
    }
    const nextStickToBottom = deriveThreadStickToBottom({
      scrollHeight: node.scrollHeight,
      scrollTop: node.scrollTop,
      clientHeight: node.clientHeight,
      wasStickToBottom: stickToBottom,
      previousScrollTop: lastActivityScrollTopRef.current,
    });
    lastActivityScrollTopRef.current = node.scrollTop;
    if (nextStickToBottom !== stickToBottom) {
      setStickToBottom(nextStickToBottom);
    }
  }, [stickToBottom]);

  return (
    <div
      className={`${TEAM_PANEL_CARD_CLASS} flex min-h-0 flex-1 flex-col overflow-hidden`}
      data-team-surface="conversation"
    >
      <div
        className="relative flex min-h-0 flex-1 flex-col overflow-hidden px-2.5 pb-2.5 pt-2 sm:px-3 sm:pb-3 sm:pt-2.5"
        data-team-channel-body="true"
      >
        {(onRefreshMessages || onLoadOlderMessages) && (
          <div className={`${TEAM_PANEL_TOOLBAR_ACTIONS_CLASS} mb-2 w-full shrink-0 justify-end gap-2`}>
            {onRefreshMessages && (
              <button
                type="button"
                className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
                onClick={() => {
                  void onRefreshMessages();
                }}
                disabled={messagesLoading || loadingOlderMessages}
                title="Refresh channel"
                aria-label="Refresh channel"
              >
                <i className="bi bi-arrow-clockwise" aria-hidden="true" />
                <span>Refresh</span>
              </button>
            )}
            {onLoadOlderMessages && (canLoadOlderMessages || loadingOlderMessages) && (
              <button
                type="button"
                className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                onClick={() => {
                  setHistoryExpanded(true);
                  setStickToBottom(false);
                  void onLoadOlderMessages();
                }}
                disabled={messagesLoading || loadingOlderMessages}
              >
                {loadingOlderMessages ? "Loading older..." : "Load older"}
              </button>
            )}
          </div>
        )}
        <div
          ref={activityListRef}
          className={activityListClassName}
          data-team-channel-scroll="true"
          onScroll={handleActivityScroll}
        >
          <div className={TEAM_TASK_ACTIVITY_SHELL_CLASS}>
            <div className={TEAM_TASK_ACTIVITY_STACK_CLASS}>
          {hiddenWaterfallSpacerHeight > 0 && (
            <div
              aria-hidden="true"
              data-team-channel-top-spacer="true"
              style={{ height: hiddenWaterfallSpacerHeight }}
            />
          )}
          {visibleWaterfallItems.map((item) => {
            const state = liveStateByMemberId.get(item.fromActorId);
            const isHumanAuthor = isHumanMailboxActor(item.fromActorId, humanActorId);
            const authorLabel = resolveThreadAuthorLabel(
              item.fromActorId,
              humanActorId,
              liveStateByMemberId
            );
            const permissionCardPayload = parsePermissionReviewCardPayload(item.payload);
            const seenActorIds = seenByMessageId[item.sequence] ?? [];
            const seenProgress = resolveSeenProgressState(
              seenActorIds,
              memberIds,
              item.fromActorId
            );
            const shouldShowSeenMeta =
              isHumanMailboxActor(item.fromActorId, humanActorId) || seenProgress.totalCount > 0;
            return (
              <div
                key={item.key}
                className={`${resolveActivityItemClassName(item.fromActorId, humanActorId)}${
                  shouldShowSeenMeta ? " pb-6 pr-7 max-[720px]:pb-5 max-[720px]:pr-6" : ""
                }`}
                data-activity-author-kind={isHumanAuthor ? "human" : "agent"}
                data-team-channel-item="true"
              >
                <div className={TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS}>
                  <div className={TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS}>
                    <span className={TEAM_TASK_ACTIVITY_AUTHOR_CLASS}>{authorLabel}</span>
                  </div>
                  <span className={TEAM_TASK_ACTIVITY_TIME_CLASS}>{formatTs(item.createdAt)}</span>
                </div>
                {permissionCardPayload ? (
                  <PermissionReviewCard
                    payload={permissionCardPayload}
                    permissionRecord={permissionRecordsById[permissionCardPayload.permission_id]}
                    busy={permissionBusyId === permissionCardPayload.permission_id}
                    errorText={permissionErrorById[permissionCardPayload.permission_id]}
                    onRespond={onRespondPermission}
                  />
                ) : isCompactCommandLikeText(item.text) ? (
                  <pre className={TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS}>{item.text}</pre>
                ) : (
                  <ThreadRichText
                    className={TEAM_TASK_ACTIVITY_BODY_CLASS}
                    text={item.text}
                    renderHtml={renderTeamMessageHtml}
                  />
                )}
                {shouldShowSeenMeta && (
                  <div className={TEAM_TASK_ACTIVITY_SEEN_META_CLASS}>
                    <HoverCard
                      openDelay={120}
                      closeDelay={80}
                      position="top-end"
                      shadow="md"
                      radius="md"
                    >
                      <HoverCard.Target>
                        {seenActorIds.length === 0 ? (
                          <button
                            type="button"
                            className={TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS}
                            aria-label="Pending delivery"
                            title="Pending delivery"
                          >
                            <span className={TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS} />
                          </button>
                        ) : (
                          <button
                            type="button"
                            className={TEAM_TASK_ACTIVITY_SEEN_BUTTON_CLASS}
                            aria-label={`Seen by ${seenProgress.readCount} of ${seenProgress.totalCount} recipients`}
                            title={`Seen by ${seenProgress.readCount} of ${seenProgress.totalCount} recipients`}
                          >
                            <span
                              className={TEAM_TASK_ACTIVITY_SEEN_DIAL_CLASS}
                              role="progressbar"
                              aria-valuenow={seenProgress.readCount}
                              aria-valuemin={0}
                              aria-valuemax={seenProgress.totalCount}
                              style={
                                {
                                  "--value": seenProgress.progress,
                                  "--size": "1rem",
                                  "--thickness": "1rem",
                                  width: "var(--size)",
                                  height: "var(--size)",
                                  background: `conic-gradient(rgba(31,122,61,0.82) calc(var(--value) * 1%), rgba(55,53,47,0.12) 0)`,
                                } satisfies SeenDialStyle
                              }
                            />
                          </button>
                        )}
                      </HoverCard.Target>
                      <HoverCard.Dropdown className={TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS}>
                        {seenActorIds.length === 0 ? (
                          <>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS}>Delivery</div>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS}>
                              Pending delivery
                            </div>
                          </>
                        ) : (
                          <>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS}>Read state</div>
                            <div className={TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS}>
                              {`${seenProgress.readCount} read · ${seenProgress.unreadCount} unread`}
                            </div>
                            {seenProgress.readActorIds.length > 0 && (
                              <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS}>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS}>Read</div>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                                  {seenProgress.readActorIds.map((actorId) => (
                                    <span
                                      key={`${item.key}-read-${actorId}`}
                                      className="rounded-full border border-ui-border bg-ui-surface px-2 py-0.5"
                                    >
                                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )}
                            {seenProgress.unreadActorIds.length > 0 && (
                              <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS}>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS}>Unread</div>
                                <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                                  {seenProgress.unreadActorIds.map((actorId) => (
                                    <span
                                      key={`${item.key}-unread-${actorId}`}
                                      className="rounded-full border border-dashed border-ui-border bg-transparent px-2 py-0.5"
                                    >
                                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                                    </span>
                                  ))}
                                </div>
                              </div>
                            )}
                          </>
                        )}
                      </HoverCard.Dropdown>
                    </HoverCard>
                  </div>
                )}
                {developerMode && (
                  <button
                    type="button"
                    className={TEAM_TASK_ACTIVITY_DETAILS_BUTTON_CLASS}
                    onClick={() =>
                      setExpandedItemKeys((current) => ({
                        ...current,
                        [item.key]: !current[item.key],
                      }))
                    }
                    aria-expanded={Boolean(expandedItemKeys[item.key])}
                  >
                    {expandedItemKeys[item.key] ? "Hide details" : "Show details"}
                  </button>
                )}
                {developerMode && expandedItemKeys[item.key] && (
                  <div className={TEAM_TASK_ACTIVITY_DETAILS_CLASS}>
                    <dl className={TEAM_TASK_ACTIVITY_DETAILS_GRID_CLASS}>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>source</dt>
                      <dd>{item.streamLabel}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>seq</dt>
                      <dd>{item.sequence}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>from</dt>
                      <dd className="mono">{item.fromActorId}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>to</dt>
                      <dd className="mono">{item.toActorId ?? "-"}</dd>
                    </div>
                    <div>
                      <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>route</dt>
                      <dd>{item.routeOrStatus}</dd>
                    </div>
                    {state && (
                      <>
                        <div>
                          <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>work</dt>
                          <dd>{state.run_status}/{state.step_status}</dd>
                        </div>
                        <div>
                          <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>agent</dt>
                          <dd>{state.lifecycle_status}</dd>
                        </div>
                        {state.current_work && (
                          <div className="sm:col-span-2">
                            <dt className={TEAM_TASK_ACTIVITY_DETAILS_LABEL_CLASS}>current_work</dt>
                            <dd>{state.current_work}</dd>
                          </div>
                        )}
                      </>
                    )}
                    </dl>
                  </div>
                )}
              </div>
            );
          })}
          {messagesLoading && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              Loading thread...
            </div>
          )}
          {!messagesLoading && orderedMessages.length === 0 && (
            <div className={TEAM_TASK_MESSAGE_EMPTY_CLASS}>
              No channel messages yet.
            </div>
          )}
            </div>
          </div>
        </div>
        {activityJumpState.showJump && (
          <button
            type="button"
            className={`${TEAM_TASK_JUMP_BUTTON_CLASS} absolute bottom-5 right-4 z-10`}
            onClick={() => {
              setStickToBottom(true);
              scrollActivityToBottom();
            }}
            title="Jump to bottom"
            aria-label="Jump to bottom"
          >
            <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
          </button>
        )}
      </div>

      <div
        className={TEAM_TASK_COMPOSER_PANEL_CLASS}
        data-team-channel-composer="true"
      >
        <textarea
          id="team-task-panel-message"
          name="team_task_message"
          ref={messageTextareaRef}
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={3}
          placeholder="Message #all"
          value={messageDraft}
          onChange={(event) => {
            const nextDraft = event.target.value;
            onMessageDraftChange(nextDraft);
            updateMentionQuery(nextDraft, event.target.selectionStart);
          }}
          onClick={(event) =>
            updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
          }
          onKeyUp={(event) =>
            updateMentionQuery(event.currentTarget.value, event.currentTarget.selectionStart)
          }
          onBlur={() => {
            setTimeout(() => {
              setActiveMention(null);
              setActiveMentionIndex(0);
            }, 0);
          }}
          onKeyDown={(event) => {
            if (activeMention && filteredMentionCandidates.length > 0) {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setActiveMentionIndex((prev) => (prev + 1) % filteredMentionCandidates.length);
                return;
              }
              if (event.key === "ArrowUp") {
                event.preventDefault();
                setActiveMentionIndex((prev) =>
                  prev === 0 ? filteredMentionCandidates.length - 1 : prev - 1
                );
                return;
              }
              if ((event.key === "Enter" || event.key === "Tab") && !event.metaKey && !event.ctrlKey) {
                event.preventDefault();
                const selected =
                  filteredMentionCandidates[activeMentionIndex] ?? filteredMentionCandidates[0];
                if (selected) {
                  applyMentionSelection(selected);
                }
                return;
              }
              if (event.key === "Escape") {
                event.preventDefault();
                setActiveMention(null);
                setActiveMentionIndex(0);
                return;
              }
            }
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canSendMessage) {
              event.preventDefault();
              sendCurrentMessage();
            }
          }}
        />
        {activeMention && filteredMentionCandidates.length > 0 && (
          <div className="mt-2 rounded-lg border border-ui-border bg-ui-surface shadow-sm">
            <div className="px-3 py-1 text-xs text-ui-text-muted">
              Select teammate mention (`@` without selection stays plain text)
            </div>
            <div className="max-h-44 overflow-auto py-1">
              {filteredMentionCandidates.map((candidate, index) => (
                <button
                  key={candidate.actorId}
                  type="button"
                  className={`flex w-full items-center justify-between px-3 py-1 text-left text-sm ${
                    index === activeMentionIndex
                      ? "bg-brand-primary/10 text-brand-primary"
                      : "text-ui-text-primary hover:bg-ui-surface-soft"
                  }`}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    applyMentionSelection(candidate);
                  }}
                >
                  <span>{candidate.label}</span>
                  <span className="text-[11px] text-ui-text-muted">{`@${candidate.label}`}</span>
                </button>
              ))}
            </div>
          </div>
        )}
        <div className={TEAM_TASK_COMPOSER_META_ROW_CLASS}>
          <span className={TEAM_TASK_SHORTCUT_CLASS}>
            {`@name for direct replies · Ctrl/Cmd + Enter sends`}
          </span>
          <button
            type="button"
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
            onClick={() => {
              sendCurrentMessage();
            }}
            disabled={!canSendMessage}
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
}

export const TeamTaskPanel = React.memo(TeamTaskPanelImpl);
TeamTaskPanel.displayName = "TeamTaskPanel";
