import { HoverCard } from "@mantine/core";
import React from "react";
import {
  type AcpPermissionOption,
  type AcpPermissionRecord,
  TeamConversationMessageRecord,
  api,
} from "../api";
import {
  DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE,
  deriveTeamThreadJumpState,
  deriveTeamThreadStickToBottom,
  windowTeamConversation,
} from "./team/team_conversation_viewport";
import { isTeamImeComposing } from "./team/team_text_helpers";
import { NOTION_FLOATING_PANEL_CLASS } from "../ui/floating_surfaces";
import {
  ActionButton,
  Badge,
  CompactButton,
  CompactIconButton,
  ConversationBubble,
  EmptyState,
  IconButton,
  KeyValueItem,
  KeyValueList,
  MenuOptionButton,
  SurfaceCard,
  ToolbarRow,
} from "../ui/primitives";
import { TeamMemberLiveState } from "./team/member_helpers";
import {
  applyMentionAtTag,
  canonicalizeMentionDraft,
  createDisplayNameLookup,
  isHumanMailboxActor,
  parseStructuredTeamPayload,
  type MentionCandidate,
  renderMarkdownWithMentions,
  resolveChatMessageText,
  resolveMentionDraftQuery,
  resolveDisplayName,
  type MentionDraftQuery,
} from "./team/mailbox_helpers";
import { TeamThreadRichText } from "./team/team_thread_rich_text";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_TASK_ACTIVITY_AUTHOR_CLASS,
  TEAM_TASK_ACTIVITY_BODY_CLASS,
  TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS,
  TEAM_TASK_ACTIVITY_CONTENT_AGENT_CLASS,
  TEAM_TASK_ACTIVITY_CONTENT_CLASS,
  TEAM_TASK_ACTIVITY_CONTENT_HUMAN_CLASS,
  TEAM_TASK_ACTIVITY_ITEM_AGENT_CLASS,
  TEAM_TASK_ACTIVITY_ITEM_HUMAN_CLASS,
  TEAM_TASK_ACTIVITY_LIST_CLASS,
  TEAM_TASK_ACTIVITY_SHELL_CLASS,
  TEAM_TASK_ACTIVITY_STACK_CLASS,
  TEAM_TASK_ACTIVITY_TIME_CLASS,
  TEAM_TASK_COMPOSER_PANEL_CLASS,
  TEAM_TASK_PERMISSION_CARD_ACTIONS_CLASS,
  TEAM_TASK_PERMISSION_CARD_BODY_CLASS,
  TEAM_TASK_PERMISSION_CARD_COMPACT_CLASS,
  TEAM_TASK_PERMISSION_CARD_COMPACT_PREVIEW_CLASS,
  TEAM_TASK_PERMISSION_CARD_HEADER_CLASS,
  TEAM_TASK_PERMISSION_CARD_REASON_CLASS,
  TEAM_TASK_PERMISSION_CARD_STATUS_CLASS,
  TEAM_TASK_PERMISSION_CARD_TITLE_CLASS,
  TEAM_TASK_PERMISSION_CARD_CLASS,
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
  messages: TeamConversationMessageRecord[];
  seenByMessageId?: Record<number, string[]>;
  humanActorId?: string;
  memberLiveStates?: TeamMemberLiveState[];
  memberIds?: string[];
  conversationTitle?: string;
  isSharedConversation?: boolean;
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

const TEAM_TASK_SHORTCUT_CLASS = "text-[11px] font-bold uppercase tracking-wider text-notion-text-muted";
const TEAM_TASK_MESSAGE_EMPTY_CLASS =
  "px-8 py-4 text-sm text-notion-text-muted italic";
const TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS = TEAM_TASK_ACTIVITY_LIST_CLASS;
const TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS =
  "mb-0.5 flex items-start justify-between gap-2";
const TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS =
  "flex min-w-0 items-center gap-2";
const TEAM_TASK_ACTIVITY_HEADER_META_CLASS = "flex shrink-0 items-center gap-2";
const TEAM_TASK_ACTIVITY_DETAILS_CLASS =
  "mt-3 rounded-xl border border-notion-border bg-notion-sidebar/10 p-3";
const TEAM_TASK_PERMISSION_CARD_ERROR_CLASS =
  "text-[11px] font-medium text-red-600";
const TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS =
  "mt-2 flex flex-wrap items-center gap-1.5 text-[11px] text-notion-text-muted";
const TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS =
  "inline-flex h-2.5 w-2.5 rounded-full bg-notion-hover ring-1 ring-notion-border";
const TEAM_TASK_ACTIVITY_SEEN_DIAL_CLASS =
  "relative inline-flex items-center justify-center overflow-hidden rounded-full align-middle";
const TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS =
  `min-w-[220px] ${NOTION_FLOATING_PANEL_CLASS}`;
const TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS =
  "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted";
const TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS =
  "mt-1 text-[13px] font-bold text-notion-text";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS = "mt-3 pt-2 border-t border-notion-border";
const TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS =
  "text-[9px] font-bold uppercase tracking-widest text-notion-text-muted";
const TEAM_TASK_TAIL_WINDOW_SIZE = DEFAULT_TEAM_CONVERSATION_TAIL_WINDOW_SIZE;
const TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT = 80;
const TEAM_TASK_ACTIVITY_BUBBLE_HUMAN_TONE_CLASS =
  "border-notion-accent/15 bg-notion-accent-bg/72";
const TEAM_TASK_ACTIVITY_BUBBLE_AGENT_TONE_CLASS =
  "border-notion-border-subtle bg-white";
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
  message: TeamConversationMessageRecord
): string | null {
  return resolveChatMessageText(message.payload);
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

function resolveActivityContentClassName(
  actorId: string,
  humanActorId: string
): string {
  return `${TEAM_TASK_ACTIVITY_CONTENT_CLASS} ${
    isHumanMailboxActor(actorId, humanActorId)
      ? TEAM_TASK_ACTIVITY_CONTENT_HUMAN_CLASS
      : TEAM_TASK_ACTIVITY_CONTENT_AGENT_CLASS
  }`;
}

function resolveActivityBubbleToneClassName(
  actorId: string,
  humanActorId: string
): string {
  return isHumanMailboxActor(actorId, humanActorId)
    ? TEAM_TASK_ACTIVITY_BUBBLE_HUMAN_TONE_CLASS
    : TEAM_TASK_ACTIVITY_BUBBLE_AGENT_TONE_CLASS;
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

function shouldHidePermissionReviewCard(
  payload: PermissionReviewCardPayload,
  record?: AcpPermissionRecord
): boolean {
  const status = resolvePermissionCardStatus(payload, record);
  if (status === "timeout") {
    return true;
  }
  const selectedOptionId = normalizeTrimmedString(record?.selected_option_id);
  return status === "responded" && Boolean(selectedOptionId);
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

type SeenProgressHoverProps = {
  itemKey: string;
  seenActorIds: string[];
  seenProgress: SeenProgressState;
  memberDisplayNamesById: Record<string, string>;
};

function SeenProgressHoverCard({
  itemKey,
  seenActorIds,
  seenProgress,
  memberDisplayNamesById,
}: SeenProgressHoverProps) {
  return (
    <HoverCard openDelay={120} closeDelay={80} position="top-end" shadow="md" radius="md">
      <HoverCard.Target>
        {seenActorIds.length === 0 ? (
          <CompactIconButton aria-label="Pending delivery" title="Pending delivery">
            <span className={TEAM_TASK_ACTIVITY_DELIVERY_PENDING_CLASS} />
          </CompactIconButton>
        ) : (
          <CompactIconButton
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
          </CompactIconButton>
        )}
      </HoverCard.Target>
      <HoverCard.Dropdown className={TEAM_TASK_ACTIVITY_SEEN_CARD_CLASS}>
        {seenActorIds.length === 0 ? (
          <>
            <div className={TEAM_TASK_ACTIVITY_SEEN_SUMMARY_CLASS}>Delivery</div>
            <div className={TEAM_TASK_ACTIVITY_SEEN_COUNT_CLASS}>Pending delivery</div>
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
                    <Badge
                      key={`${itemKey}-read-${actorId}`}
                      tone="outline"
                      shape="pill"
                      className="text-[11px]"
                    >
                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
            {seenProgress.unreadActorIds.length > 0 && (
              <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_CLASS}>
                <div className={TEAM_TASK_ACTIVITY_SEEN_SECTION_TITLE_CLASS}>Unread</div>
                <div className={TEAM_TASK_ACTIVITY_SEEN_LIST_CLASS}>
                  {seenProgress.unreadActorIds.map((actorId) => (
                    <Badge
                      key={`${itemKey}-unread-${actorId}`}
                      tone="dashed"
                      shape="pill"
                      className="text-[11px]"
                    >
                      {resolveDisplayName(actorId, memberDisplayNamesById, actorId)}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </HoverCard.Dropdown>
    </HoverCard>
  );
}

type ActivityDetailsPanelProps = {
  item: {
    streamLabel: string;
    sequence: number;
    fromActorId: string;
    toActorId?: string | null;
    routeOrStatus: string;
  };
  state?: TeamMemberLiveState;
};

function ActivityDetailsPanel({ item, state }: ActivityDetailsPanelProps) {
  return (
    <div className={TEAM_TASK_ACTIVITY_DETAILS_CLASS}>
      <KeyValueList>
        <KeyValueItem label="source" value={item.streamLabel} />
        <KeyValueItem label="seq" value={item.sequence} />
        <KeyValueItem label="from" value={item.fromActorId} valueClassName="mono" />
        <KeyValueItem label="to" value={item.toActorId ?? "-"} valueClassName="mono" />
        <KeyValueItem label="route" value={item.routeOrStatus} />
        {state ? (
          <>
            <KeyValueItem label="work" value={`${state.run_status}/${state.step_status}`} />
            <KeyValueItem label="agent" value={state.lifecycle_status} />
            {state.current_work ? (
              <KeyValueItem label="current_work" value={state.current_work} />
            ) : null}
          </>
        ) : null}
      </KeyValueList>
    </div>
  );
}

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
        <Badge className={TEAM_TASK_PERMISSION_CARD_STATUS_CLASS}>{statusText}</Badge>
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
                  <ActionButton
                    key={`${optionId}:${index}`}
                    tone="primary"
                    size="sm"
                    disabled={busy || !optionId}
                    onClick={() => onRespond(payload, optionId)}
                  >
                    {option.name}
                  </ActionButton>
                );
              })}
              <ActionButton
                tone="secondary"
                size="sm"
                disabled={busy}
                onClick={() => onRespond(payload)}
              >
                Cancel
              </ActionButton>
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
    onRefreshMessages,
    onSendMessage,
    messages,
    seenByMessageId = {},
    humanActorId = "user",
    memberLiveStates = [],
    memberIds = [],
    conversationTitle = "all",
    isSharedConversation = true,
    messagesLoading,
    busy,
    formatTs,
  } = props;

  const messageTextareaRef = React.useRef<HTMLTextAreaElement | null>(null);
  const messageDraftComposingRef = React.useRef(false);
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
  const normalizedConversationTitle =
    conversationTitle.trim().length > 0 ? conversationTitle.trim() : "all";
  const refreshLabel = isSharedConversation ? "Refresh channel" : "Refresh thread";
  const emptyStateText = isSharedConversation
    ? "No channel messages yet."
    : "No thread messages yet.";
  const messagePlaceholder = isSharedConversation
    ? `Message #${normalizedConversationTitle}`
    : "Reply in thread";

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
        const record = payload ? permissionRecordsById[payload.permission_id] : undefined;
        return payload
          && !shouldHidePermissionReviewCard(payload, record)
          ? [{ permissionId: payload.permission_id, agentId: payload.agent_id }]
          : [];
      }),
    [orderedMessages, permissionRecordsById]
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
      const record = permissionRecordsById[payload.permission_id];
      if (shouldHidePermissionReviewCard(payload, record)) {
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
      windowTeamConversation(
        orderedMessages.flatMap((message) => {
          const permissionCardPayload = parsePermissionReviewCardPayload(message.payload);
          if (permissionCardPayload) {
            const permissionRecord = permissionRecordsById[permissionCardPayload.permission_id];
            if (shouldHidePermissionReviewCard(permissionCardPayload, permissionRecord)) {
              return [];
            }
            return [
              {
                message,
                permissionCardPayload,
                text: "",
              },
            ];
          }
          const visibleText = resolveMessageText(message);
          if (visibleText === null) {
            return [];
          }
          return [
            {
              message,
              permissionCardPayload: null,
              text: visibleText,
            },
          ];
        }),
        stickToBottom,
        TEAM_TASK_TAIL_WINDOW_SIZE
      ),
    [orderedMessages, permissionRecordsById, stickToBottom]
  );
  const visibleWaterfallItems = React.useMemo(
    () =>
      activityWindow.items.map((item) => {
        const message = item.message;
        return {
          key: `conversation-${message.message_id}`,
          sequence: message.message_id,
          createdAt: message.created_at,
          fromActorId: message.from_actor_id,
          toActorId: message.to_actor_id ?? null,
          routeOrStatus: message.route,
          streamLabel: "conversation",
          payload: message.payload,
          text: item.text,
          permissionCardPayload: item.permissionCardPayload,
        };
      }),
    [activityWindow.items]
  );
  const hiddenWaterfallCount = activityWindow.offset;
  const hiddenWaterfallSpacerHeight = React.useMemo(() => {
    if (!stickToBottom || hiddenWaterfallCount <= 0) {
      return 0;
    }
    return hiddenWaterfallCount * TEAM_TASK_TAIL_WINDOW_ESTIMATED_ITEM_HEIGHT;
  }, [hiddenWaterfallCount, stickToBottom]);
  const activityListClassName =
    messagesLoading || visibleWaterfallItems.length > 0
      ? TEAM_TASK_ACTIVITY_LIST_CLASS
      : TEAM_TASK_ACTIVITY_LIST_EMPTY_CLASS;
  const showInitialThreadLoading = messagesLoading && visibleWaterfallItems.length === 0;
  const latestWaterfallKey =
    visibleWaterfallItems.length > 0
      ? visibleWaterfallItems[visibleWaterfallItems.length - 1]?.key ?? "empty"
      : "empty";
  const activityJumpState = React.useMemo(
    () =>
      deriveTeamThreadJumpState({
        active: visibleWaterfallItems.length > 0,
        stickToBottom,
        pendingCount: 0,
    }),
    [stickToBottom, visibleWaterfallItems.length]
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
    const nextStickToBottom = deriveTeamThreadStickToBottom({
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
    <SurfaceCard
      className={`${TEAM_PANEL_CARD_CLASS} flex min-h-0 flex-1 flex-col overflow-hidden`}
      data-team-surface="conversation"
    >
      <div
        className="relative flex min-h-0 flex-1 flex-col overflow-hidden px-2.5 pb-2.5 pt-2 sm:px-3 sm:pb-3 sm:pt-2.5"
        data-team-channel-body="true"
      >
        {onRefreshMessages && (
          <ToolbarRow className="mb-2 w-full shrink-0 justify-end gap-2">
            <ActionButton
              tone="secondary"
              size="md"
              onClick={() => {
                void onRefreshMessages();
              }}
              disabled={messagesLoading}
              title={refreshLabel}
              aria-label={refreshLabel}
            >
              <i className="bi bi-arrow-clockwise" aria-hidden="true" />
              <span>Refresh</span>
            </ActionButton>
          </ToolbarRow>
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
            const permissionCardPayload = item.permissionCardPayload;
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
                className={resolveActivityItemClassName(item.fromActorId, humanActorId)}
                data-activity-author-kind={isHumanAuthor ? "human" : "agent"}
                data-team-channel-item="true"
              >
                <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-sm text-[10px] font-bold uppercase tracking-tight shadow-sm mt-0.5 ${!isHumanAuthor ? "bg-notion-accent text-white" : "bg-notion-hover text-notion-text-muted"}`}>
                  {isHumanAuthor ? "U" : authorLabel.charAt(0).toUpperCase()}
                </div>
                <div className={resolveActivityContentClassName(item.fromActorId, humanActorId)}>
                  <div className={TEAM_TASK_ACTIVITY_HEADER_ROW_CLASS}>
                    <div className={TEAM_TASK_ACTIVITY_AUTHOR_ROW_CLASS}>
                      <span className={TEAM_TASK_ACTIVITY_AUTHOR_CLASS}>{authorLabel}</span>
                      <span className={TEAM_TASK_ACTIVITY_TIME_CLASS}>{formatTs(item.createdAt)}</span>
                    </div>
                    {(shouldShowSeenMeta || developerMode) && (
                      <div className={TEAM_TASK_ACTIVITY_HEADER_META_CLASS}>
                        {shouldShowSeenMeta && (
                          <SeenProgressHoverCard
                            itemKey={item.key}
                            seenActorIds={seenActorIds}
                            seenProgress={seenProgress}
                            memberDisplayNamesById={memberDisplayNamesById}
                          />
                        )}
                        {developerMode && (
                          <CompactButton
                            onClick={() =>
                              setExpandedItemKeys((current) => ({
                                ...current,
                                [item.key]: !current[item.key],
                              }))
                            }
                            aria-expanded={Boolean(expandedItemKeys[item.key])}
                          >
                            {expandedItemKeys[item.key] ? "Hide details" : "Show details"}
                          </CompactButton>
                        )}
                      </div>
                    )}
                  </div>
                  {permissionCardPayload ? (
                    <div data-team-channel-bubble="permission" className="mt-1 max-w-full">
                      <PermissionReviewCard
                        payload={permissionCardPayload}
                        permissionRecord={permissionRecordsById[permissionCardPayload.permission_id]}
                        busy={permissionBusyId === permissionCardPayload.permission_id}
                        errorText={permissionErrorById[permissionCardPayload.permission_id]}
                        onRespond={onRespondPermission}
                      />
                    </div>
                  ) : isCompactCommandLikeText(item.text) ? (
                    <ConversationBubble
                      data-team-channel-bubble={isHumanAuthor ? "human" : "agent"}
                      className={resolveActivityBubbleToneClassName(item.fromActorId, humanActorId)}
                    >
                      <pre className={TEAM_TASK_ACTIVITY_COMMAND_BODY_CLASS}>{item.text}</pre>
                    </ConversationBubble>
                  ) : (
                    <ConversationBubble
                      data-team-channel-bubble={isHumanAuthor ? "human" : "agent"}
                      className={resolveActivityBubbleToneClassName(item.fromActorId, humanActorId)}
                    >
                      <TeamThreadRichText
                        className={TEAM_TASK_ACTIVITY_BODY_CLASS}
                        text={item.text}
                        renderSanitizedHtml={renderTeamMessageHtml}
                      />
                    </ConversationBubble>
                  )}
                  {developerMode && expandedItemKeys[item.key] && (
                    <ActivityDetailsPanel item={item} state={state} />
                  )}
                </div>
              </div>
            );
          })}
          {showInitialThreadLoading && (
            <EmptyState title="Loading thread..." className={TEAM_TASK_MESSAGE_EMPTY_CLASS} />
          )}
          {!showInitialThreadLoading && visibleWaterfallItems.length === 0 && (
            <EmptyState title={emptyStateText} className={TEAM_TASK_MESSAGE_EMPTY_CLASS} />
          )}
            </div>
          </div>
        </div>
        {activityJumpState.showJump && (
          <IconButton
            tone="default"
            size="md"
            className="absolute bottom-5 right-4 z-10 h-9 w-9 rounded-full border border-notion-border bg-white text-notion-text-muted shadow-md hover:bg-notion-hover hover:text-notion-text"
            onClick={() => {
              setStickToBottom(true);
              scrollActivityToBottom();
            }}
            title="Jump to bottom"
            aria-label="Jump to bottom"
          >
            <i className="bi bi-chevron-down text-sm" aria-hidden="true" />
          </IconButton>
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
          className={`${TEAM_PANEL_TEXTAREA_CLASS} min-h-[40px] px-2.5 py-1.5 text-[13px] leading-5`}
          rows={1}
          placeholder={messagePlaceholder}
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
          onCompositionStart={() => {
            messageDraftComposingRef.current = true;
          }}
          onCompositionEnd={() => {
            messageDraftComposingRef.current = false;
          }}
          onKeyDown={(event) => {
            const composing = isTeamImeComposing(
              messageDraftComposingRef.current,
              event.nativeEvent.isComposing,
              "keyCode" in event.nativeEvent
                ? Number((event.nativeEvent as KeyboardEvent).keyCode)
                : undefined
            );
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
            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.altKey &&
              !event.metaKey &&
              !event.ctrlKey &&
              !composing &&
              canSendMessage
            ) {
              event.preventDefault();
              sendCurrentMessage();
              return;
            }
          }}
        />
        {activeMention && filteredMentionCandidates.length > 0 && (
          <div className="mt-2 overflow-hidden rounded-xl border border-ui-border bg-ui-surface shadow-sm">
            <div className="px-3 py-1 text-xs text-ui-text-muted">
              Select teammate mention (`@` without selection stays plain text)
            </div>
            <div className="max-h-44 overflow-auto py-1">
              {filteredMentionCandidates.map((candidate, index) => (
                <MenuOptionButton
                  key={candidate.actorId}
                  active={index === activeMentionIndex}
                  data-team-mention-option={candidate.actorId}
                  onMouseDown={(event) => {
                    event.preventDefault();
                    applyMentionSelection(candidate);
                  }}
                >
                  <span>{candidate.label}</span>
                  <span className="text-[11px] text-ui-text-muted">{`@${candidate.label}`}</span>
                </MenuOptionButton>
              ))}
            </div>
          </div>
        )}
        <ToolbarRow className="mt-0.5 gap-2">
          <span className={TEAM_TASK_SHORTCUT_CLASS}>
            {`@name for direct replies · Enter sends · Shift/Ctrl/Cmd + Enter newline`}
          </span>
          <ActionButton
            tone="primary"
            size="sm"
            className="px-3.5"
            onClick={() => {
              sendCurrentMessage();
            }}
            disabled={!canSendMessage}
          >
            Send
          </ActionButton>
        </ToolbarRow>
      </div>
    </SurfaceCard>
  );
}

export const TeamTaskPanel = React.memo(TeamTaskPanelImpl);
TeamTaskPanel.displayName = "TeamTaskPanel";
