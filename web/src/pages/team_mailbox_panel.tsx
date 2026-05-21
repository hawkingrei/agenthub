import React from "react";
import {
  TeamActorMessageRecord,
  TeamReplyObligationRecord,
  TeamRunSnapshotRecord,
} from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  ActionButton,
  EmptyState,
  InlineNotice,
  InsetSurface,
  PanelHeader,
  SelectableListItem,
  SurfaceCard,
  ToolbarRow,
} from "../ui/primitives";
import {
  isHumanMailboxActor,
  resolveDisplayName,
  renderPlainTextWithMentions,
  resolveVisibleTeamPayloadText,
} from "./team/mailbox_helpers";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  MAILBOX_ADVANCED_HINT_CLASS,
  MAILBOX_META_CLASS,
  MAILBOX_META_LABEL_CLASS,
  MAILBOX_META_VALUE_CLASS,
  MAILBOX_SHELL_CLASS,
  MAILBOX_MEMBER_LIST_CLASS,
  MAILBOX_SECTION_TITLE_CLASS,
  MAILBOX_MEMBER_ROW_HEADER_CLASS,
  MAILBOX_MEMBER_ROW_META_CLASS,
  MAILBOX_MEMBER_UNREAD_BADGE_CLASS,
  MAILBOX_PANEL_CLASS,
  MAILBOX_CHAT_HEADER_CLASS,
  MAILBOX_CHAT_TITLE_CLASS,
  MAILBOX_CHAT_SUBTITLE_CLASS,
  MAILBOX_CHAT_STATUS_CLASS,
  MAILBOX_CHAT_JUMP_BUTTON_CLASS,
  MAILBOX_MESSAGE_LIST_CLASS,
  MAILBOX_MESSAGE_ITEM_CLASS,
  MAILBOX_MESSAGE_BUBBLE_OUTGOING_CLASS,
  MAILBOX_MESSAGE_BUBBLE_INCOMING_CLASS,
  MAILBOX_CONVERSATION_EMPTY_CLASS,
  MAILBOX_MESSAGE_HEAD_CLASS,
  MAILBOX_MESSAGE_BODY_CLASS,
  MAILBOX_MESSAGE_PRE_CLASS,
  MAILBOX_MESSAGE_ACTIONS_CLASS,
  MAILBOX_COMPOSER_CLASS,
  MAILBOX_COMPOSER_TEXTAREA_CLASS,
  MAILBOX_ADVANCED_GRID_CLASS,
  MAILBOX_ADVANCED_PANEL_CLASS,
  MAILBOX_ADVANCED_PANEL_TITLE_CLASS,
  MAILBOX_ADVANCED_TEMPLATE_ROW_CLASS,
  MAILBOX_CHECKBOX_LABEL_CLASS,
  MAILBOX_ADVANCED_ROOT_CLASS,
  MAILBOX_ADVANCED_TITLE_CLASS,
} from "../ui/tailwind_classes";

type ChatActors = {
  fromActorId: string;
  toActorId: string;
  inboxActorId: string;
};

type MailboxTemplateOption = {
  value: string;
  label: string;
};

type MailboxActorRow = {
  memberId: string;
  role: string;
  pendingInboxCount: number;
  replyObligationCount: number;
  status: string;
  unread: number;
  isHuman: boolean;
  label: string;
};

type MailboxConversationRow = {
  message: TeamActorMessageRecord;
  isOutgoing: boolean;
  htmlPayload: string | null;
  payload: string;
  fromLabel: string;
  toLabel: string;
  canAccept: boolean;
};

type TeamMailboxPanelProps = {
  developerMode: boolean;
  mode?: "full" | "advanced_only";
  snapshot: TeamRunSnapshotRecord | null;
  humanActorId?: string;
  displayNameByActorId?: Record<string, string>;
  selectedMemberId: string;
  unreadByMemberId: Record<string, number>;
  onSelectMember: (memberId: string) => void;
  onOpenMemberProfile?: (memberId: string) => void;
  chatActors: ChatActors;
  chatStickToBottom: boolean;
  chatMessagesRef: React.RefObject<HTMLUListElement | null>;
  onConversationScroll: () => void;
  onJumpToBottom: () => void;
  conversationMessages: TeamActorMessageRecord[];
  toPrettyJson: (value: unknown) => string;
  formatTs: (ts?: number | null) => string;
  busy: string | null;
  onAcceptMessage: (message: TeamActorMessageRecord) => Promise<void> | void;
  onAcceptVisibleMessages?: (messages: TeamActorMessageRecord[]) => Promise<void> | void;
  onTriageMessage?: (
    message: TeamActorMessageRecord,
    disposition: "ignored" | "watching" | "claimed" | "completed" | "released"
  ) => Promise<void> | void;
  onEscalateMessage?: (message: TeamActorMessageRecord) => Promise<void> | void;
  chatDraft: string;
  onChatDraftChange: (value: string) => void;
  onSendChatMessage: () => Promise<void> | void;
  msgFromActorId: string;
  onMsgFromActorIdChange: (value: string) => void;
  msgToActorId: string;
  onMsgToActorIdChange: (value: string) => void;
  msgChannel: string;
  onMsgChannelChange: (value: string) => void;
  msgTransport: "local" | "remote";
  onMsgTransportChange: (value: "local" | "remote") => void;
  msgRoute: string;
  onMsgRouteChange: (value: string) => void;
  mailboxTemplateOptions: MailboxTemplateOption[];
  msgTemplate: string;
  onMsgTemplateChange: (value: string) => void;
  onApplyMessageTemplate: () => void;
  msgPayload: string;
  onMsgPayloadChange: (value: string) => void;
  msgIdempotencyKey: string;
  onMsgIdempotencyKeyChange: (value: string) => void;
  onSendMessage: () => Promise<void> | void;
  inboxActorId: string;
  onInboxActorIdChange: (value: string) => void;
  inboxLimit: string;
  onInboxLimitChange: (value: string) => void;
  inboxAfterId: string;
  onInboxAfterIdChange: (value: string) => void;
  inboxIncludeDelivered: boolean;
  onInboxIncludeDeliveredChange: (value: boolean) => void;
  onRefreshInbox: () => Promise<void> | void;
};

function resolveMailboxActorLabel(
  actorId: string,
  displayNameByActorId: Record<string, string>,
  humanActorId: string
): string {
  const normalizedActorId = actorId.trim();
  if (!normalizedActorId) {
    return "-";
  }
  if (isHumanMailboxActor(normalizedActorId, humanActorId)) {
    return "You";
  }
  return resolveDisplayName(normalizedActorId, displayNameByActorId, normalizedActorId);
}

function isMessageAcceptableForInbox(
  message: TeamActorMessageRecord,
  inboxActorId: string,
  humanActorId: string
): boolean {
  if (message.status !== "pending") {
    return false;
  }
  const normalizedInboxActorId = inboxActorId.trim();
  if (!normalizedInboxActorId) {
    return false;
  }
  if (message.to_actor_id === normalizedInboxActorId) {
    return true;
  }
  return (
    isHumanMailboxActor(message.to_actor_id, humanActorId) &&
    isHumanMailboxActor(normalizedInboxActorId, humanActorId)
  );
}

function resolveMentionActorIdFromEventTarget(target: EventTarget | null): string | null {
  const mention = (target as HTMLElement | null)?.closest("[data-team-agent-mention-id]");
  const actorId = mention?.getAttribute("data-team-agent-mention-id")?.trim();
  return actorId || null;
}

function resolveMailboxDispositionLabel(message: TeamActorMessageRecord | null): string {
  return message?.handling_disposition ?? "untriaged";
}

function TeamMailboxPanelImpl(props: TeamMailboxPanelProps) {
  const {
    mode = "full",
    developerMode,
    snapshot,
    humanActorId = "",
    displayNameByActorId = {},
    selectedMemberId,
    unreadByMemberId,
    onSelectMember,
    onOpenMemberProfile,
    chatActors,
    chatStickToBottom,
    chatMessagesRef,
    onConversationScroll,
    onJumpToBottom,
    conversationMessages,
    toPrettyJson,
    formatTs,
    busy,
    onAcceptMessage,
    onAcceptVisibleMessages,
    onTriageMessage,
    onEscalateMessage,
    chatDraft,
    onChatDraftChange,
    onSendChatMessage,
    msgFromActorId,
    onMsgFromActorIdChange,
    msgToActorId,
    onMsgToActorIdChange,
    msgChannel,
    onMsgChannelChange,
    msgTransport,
    onMsgTransportChange,
    msgRoute,
    onMsgRouteChange,
    mailboxTemplateOptions,
    msgTemplate,
    onMsgTemplateChange,
    onApplyMessageTemplate,
    msgPayload,
    onMsgPayloadChange,
    msgIdempotencyKey,
    onMsgIdempotencyKeyChange,
    onSendMessage,
    inboxActorId,
    onInboxActorIdChange,
    inboxLimit,
    onInboxLimitChange,
    inboxAfterId,
    onInboxAfterIdChange,
    inboxIncludeDelivered,
    onInboxIncludeDeliveredChange,
    onRefreshInbox,
  } = props;
  const showConversation = mode === "full";
  const showAdvancedControls = mode === "advanced_only";
  const showDeveloperMailboxTools = developerMode && showAdvancedControls;
  const normalizedHumanActorId = humanActorId.trim();
  const openReplyObligations = snapshot?.mailbox.open_reply_obligations ?? [];
  const mailboxMessagesById = React.useMemo(
    () => new Map(snapshot?.mailbox.recent_messages.map((message) => [message.message_id, message]) ?? []),
    [snapshot?.mailbox.recent_messages]
  );
  const obligationRows = React.useMemo<
    Array<{ obligation: TeamReplyObligationRecord; message: TeamActorMessageRecord | null }>
  >(
    () =>
      openReplyObligations.map((obligation) => ({
        obligation,
        message: mailboxMessagesById.get(obligation.message_id) ?? null,
      })),
    [mailboxMessagesById, openReplyObligations]
  );
  const humanPendingCount =
    snapshot?.mailbox.recent_messages.filter(
      (message) =>
        message.to_actor_id === normalizedHumanActorId &&
        message.status === "pending"
    ).length ?? 0;
  const mailboxActorRows = React.useMemo<MailboxActorRow[]>(() => {
    const mailboxMembers = snapshot?.members ?? [];
    const mailboxActors = [
      ...mailboxMembers,
      ...(normalizedHumanActorId &&
      !mailboxMembers.some((member) => member.member_id === normalizedHumanActorId)
        ? [
            {
              member_id: normalizedHumanActorId,
              role: "human",
              pending_inbox_count: humanPendingCount,
              reply_obligation_count: 0,
              status: "human",
            },
          ]
        : []),
    ];
    return mailboxActors.map((member) => {
      const isHuman = member.role === "human";
      return {
        memberId: member.member_id,
        role: member.role,
        pendingInboxCount: member.pending_inbox_count,
        replyObligationCount: member.reply_obligation_count ?? 0,
        status: member.status,
        unread: unreadByMemberId[member.member_id] ?? 0,
        isHuman,
        label: resolveMailboxActorLabel(
          member.member_id,
          displayNameByActorId,
          normalizedHumanActorId
        ),
      };
    });
  }, [
    displayNameByActorId,
    humanPendingCount,
    normalizedHumanActorId,
    snapshot?.members,
    unreadByMemberId,
  ]);
  const conversationRows = React.useMemo<MailboxConversationRow[]>(() => {
    // Precompute mailbox row presentation so long conversations do not repeatedly
    // re-parse payload text and actor labels during every list render.
    return conversationMessages.map((message) => {
      const isOutgoing = message.from_actor_id === chatActors.fromActorId;
      const chatText = resolveVisibleTeamPayloadText(message.payload);
      const payload = chatText ?? toPrettyJson(message.payload);
      const htmlPayload =
        chatText === null
          ? null
          : renderPlainTextWithMentions(payload, displayNameByActorId);
      return {
        message,
        isOutgoing,
        htmlPayload,
        payload,
        fromLabel: resolveMailboxActorLabel(
          message.from_actor_id,
          displayNameByActorId,
          normalizedHumanActorId
        ),
        toLabel: resolveMailboxActorLabel(
          message.to_actor_id,
          displayNameByActorId,
          normalizedHumanActorId
        ),
        canAccept: isMessageAcceptableForInbox(
          message,
          chatActors.inboxActorId,
          normalizedHumanActorId
        ),
      };
    });
  }, [
    chatActors.fromActorId,
    chatActors.inboxActorId,
    conversationMessages,
    displayNameByActorId,
    normalizedHumanActorId,
    toPrettyJson,
  ]);
  const acceptVisibleMessages = React.useMemo(
    () => conversationRows.filter((row) => row.canAccept).map((row) => row.message),
    [conversationRows]
  );

  const advancedControls = (
    <div className={MAILBOX_ADVANCED_GRID_CLASS}>
      <div className={MAILBOX_ADVANCED_PANEL_CLASS}>
        <h4 className={MAILBOX_ADVANCED_PANEL_TITLE_CLASS}>Send Message (JSON)</h4>
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="from_actor_id"
          value={msgFromActorId}
          onChange={(event) => onMsgFromActorIdChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="to_actor_id"
          value={msgToActorId}
          onChange={(event) => onMsgToActorIdChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="channel (default)"
          value={msgChannel}
          onChange={(event) => onMsgChannelChange(event.target.value)}
        />
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={msgTransport}
          onChange={(event) => onMsgTransportChange(event.target.value as "local" | "remote")}
        >
          <option value="local">local</option>
          <option value="remote">remote</option>
        </select>
        <textarea
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={3}
          placeholder="route JSON (required for remote)"
          value={msgRoute}
          onChange={(event) => onMsgRouteChange(event.target.value)}
        />
        <div className={MAILBOX_ADVANCED_TEMPLATE_ROW_CLASS}>
          <select
            className={TEAM_PANEL_INPUT_CLASS}
            value={msgTemplate}
            onChange={(event) => onMsgTemplateChange(event.target.value)}
          >
            {mailboxTemplateOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
          <ActionButton tone="secondary" size="md" onClick={onApplyMessageTemplate}>
            Apply Template
          </ActionButton>
        </div>
        <textarea
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={4}
          placeholder="payload JSON"
          value={msgPayload}
          onChange={(event) => onMsgPayloadChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="idempotency_key (optional)"
          value={msgIdempotencyKey}
          onChange={(event) => onMsgIdempotencyKeyChange(event.target.value)}
        />
        <ActionButton
          tone="primary"
          size="md"
          onClick={onSendMessage}
          disabled={busy === "send-message"}
        >
          Send Message
        </ActionButton>
      </div>

      <div className={MAILBOX_ADVANCED_PANEL_CLASS}>
        <h4 className={MAILBOX_ADVANCED_PANEL_TITLE_CLASS}>Inbox Query (read-only)</h4>
        <p className={TEAM_MUTED_TEXT_CLASS}>
          Refresh inspects mailbox state only. Accept pending work from the conversation pane when
          you are taking ownership of it.
        </p>
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="actor_id"
          value={inboxActorId}
          onChange={(event) => onInboxActorIdChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="limit"
          value={inboxLimit}
          onChange={(event) => onInboxLimitChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="after_id (optional)"
          value={inboxAfterId}
          onChange={(event) => onInboxAfterIdChange(event.target.value)}
        />
        <label className={MAILBOX_CHECKBOX_LABEL_CLASS}>
          <input
            type="checkbox"
            checked={inboxIncludeDelivered}
            onChange={(event) => onInboxIncludeDeliveredChange(event.target.checked)}
          />
          include_delivered
        </label>
        <ActionButton
          tone="secondary"
          size="md"
          onClick={onRefreshInbox}
          disabled={busy !== null}
          title="Refresh read-only inbox"
          aria-label="Refresh read-only inbox"
        >
          <i className="bi bi-arrow-clockwise" aria-hidden="true" />
          <span>Refresh Query</span>
        </ActionButton>
      </div>
    </div>
  );

  return (
    <SurfaceCard className={`${TEAM_PANEL_CARD_CLASS} p-3`}>
      <PanelHeader title="Mailbox" titleClassName={TEAM_PANEL_TITLE_CLASS} />

      {showConversation && snapshot && (
        <>
          <div className={MAILBOX_META_CLASS}>
            <span>
              <strong className={MAILBOX_META_LABEL_CLASS}>Pending:</strong>
              <span className={MAILBOX_META_VALUE_CLASS}>{snapshot.mailbox.pending}</span>
            </span>
            <span>
              <strong className={MAILBOX_META_LABEL_CLASS}>Delivered:</strong>
              <span className={MAILBOX_META_VALUE_CLASS}>{snapshot.mailbox.delivered}</span>
            </span>
            <span>
              <strong className={MAILBOX_META_LABEL_CLASS}>Dead Letter:</strong>
              <span className={MAILBOX_META_VALUE_CLASS}>{snapshot.mailbox.dead_letter}</span>
            </span>
            <span>
              <strong className={MAILBOX_META_LABEL_CLASS}>Reply Obligations:</strong>
              <span className={MAILBOX_META_VALUE_CLASS}>
                {snapshot.mailbox.open_reply_obligation_count ?? 0}
              </span>
            </span>
            <span>
              <strong className={MAILBOX_META_LABEL_CLASS}>Recent Messages:</strong>
              <span className={MAILBOX_META_VALUE_CLASS}>{snapshot.mailbox.recent_messages.length}</span>
            </span>
          </div>
          {openReplyObligations.length > 0 ? (
            <div className="mb-3 rounded-lg border border-notion-border/60 bg-notion-sidebar/5 p-3">
              <h4 className="text-[12px] font-semibold uppercase tracking-[0.08em] text-notion-text-muted">
                Open Reply Obligations
              </h4>
              <ul className="mt-2 flex flex-col gap-2 text-[12px] leading-relaxed text-notion-text">
                {obligationRows.map(({ obligation, message }) => {
                  const disposition = resolveMailboxDispositionLabel(message);
                  const claimStatus = message?.thread_claim_status ?? null;
                  const claimOwner = message?.thread_owner_actor_id?.trim() ?? "";
                  const canRelease =
                    !!message &&
                    (message.thread_claim_status === "claimed" ||
                      message.handling_disposition === "claimed") &&
                    (!claimOwner || claimOwner === obligation.agent_actor_id);
                  const canTakeOver = !!message && message.handling_disposition !== "claimed";
                  const canWatch = !!message && message.handling_disposition !== "watching";
                  const canEscalate =
                    !!message &&
                    !!snapshot?.coordinator_member_id &&
                    obligation.agent_actor_id !== snapshot.coordinator_member_id;
                  return (
                    <li key={obligation.message_id} className="rounded-md bg-white px-3 py-2">
                    <div className="font-medium">
                      {resolveMailboxActorLabel(
                        obligation.agent_actor_id,
                        displayNameByActorId,
                        normalizedHumanActorId
                      )}{" "}
                      owes{" "}
                      {resolveMailboxActorLabel(
                        obligation.human_actor_id,
                        displayNameByActorId,
                        normalizedHumanActorId
                      )}{" "}
                      a reply
                    </div>
                    <div className="text-notion-text-muted">
                      state={disposition}
                      {claimStatus ? ` · claim=${claimStatus}` : ""}
                      {claimOwner
                        ? ` · owner=${resolveMailboxActorLabel(
                            claimOwner,
                            displayNameByActorId,
                            normalizedHumanActorId
                          )}`
                        : ""}
                      {" · "}source={obligation.source_surface} · ts={formatTs(obligation.created_at)}
                    </div>
                    {obligation.text_excerpt ? (
                      <div className="mt-1 whitespace-pre-wrap break-words text-notion-text-muted">
                        {obligation.text_excerpt}
                      </div>
                    ) : null}
                    <ToolbarRow className="mt-2 justify-end gap-2">
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => onSelectMember(obligation.agent_actor_id)}
                        disabled={busy !== null}
                      >
                        Open mailbox
                      </ActionButton>
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => {
                          if (message) {
                            void onTriageMessage?.(message, "watching");
                          }
                        }}
                        disabled={busy !== null || !canWatch || !onTriageMessage}
                      >
                        Watch
                      </ActionButton>
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => {
                          if (message) {
                            void onTriageMessage?.(message, "claimed");
                          }
                        }}
                        disabled={busy !== null || !canTakeOver || !onTriageMessage}
                      >
                        Take over
                      </ActionButton>
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => {
                          if (message) {
                            void onTriageMessage?.(message, "released");
                          }
                        }}
                        disabled={busy !== null || !canRelease || !onTriageMessage}
                      >
                        Release
                      </ActionButton>
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => {
                          if (message) {
                            void onEscalateMessage?.(message);
                          }
                        }}
                        disabled={busy !== null || !canEscalate || !onEscalateMessage}
                      >
                        Escalate
                      </ActionButton>
                      <ActionButton
                        tone="secondary"
                        size="sm"
                        onClick={() => {
                          if (message) {
                            void onTriageMessage?.(message, "ignored");
                          }
                        }}
                        disabled={busy !== null || !message || !onTriageMessage}
                      >
                        Ignore
                      </ActionButton>
                    </ToolbarRow>
                  </li>
                );
              })}
              </ul>
            </div>
          ) : null}
        </>
      )}

      {showConversation && (
        <div className={MAILBOX_SHELL_CLASS}>
          <div className={MAILBOX_MEMBER_LIST_CLASS}>
            <h4 className={MAILBOX_SECTION_TITLE_CLASS}>Agents</h4>
            {mailboxActorRows.map((member) => {
              return (
                <SelectableListItem
                  key={member.memberId}
                  active={selectedMemberId === member.memberId}
                  onClick={() => onSelectMember(member.memberId)}
                >
                  <div className={MAILBOX_MEMBER_ROW_HEADER_CLASS}>
                    <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} font-bold`}>
                      {member.label} ({member.role})
                    </span>
                    {!member.isHuman && (
                      <StatusBadge
                        label={member.status}
                        tone={resolveTeamRunStatusTone(member.status)}
                        className="team-status"
                        title={`member status: ${member.status}`}
                      />
                    )}
                  </div>
                  <div className={MAILBOX_MEMBER_ROW_META_CLASS}>
                    <span className={TEAM_LIST_ITEM_META_CLASS}>
                      {member.isHuman
                        ? "human actor"
                        : `pending=${member.pendingInboxCount} reply=${member.replyObligationCount}`}
                    </span>
                    {member.unread > 0 && (
                      <span className={MAILBOX_MEMBER_UNREAD_BADGE_CLASS}>
                        unread={member.unread}
                      </span>
                    )}
                  </div>
                </SelectableListItem>
              );
            })}
            {mailboxActorRows.length === 0 && (
              <EmptyState className={`${TEAM_MUTED_TEXT_CLASS} px-2`} body="No members available." />
            )}
          </div>

          <div className={MAILBOX_PANEL_CLASS}>
            <ToolbarRow className={MAILBOX_CHAT_HEADER_CLASS}>
              <div className="min-w-0 flex-1">
                <span className={MAILBOX_CHAT_TITLE_CLASS}>Conversation</span>
                <div className={MAILBOX_CHAT_SUBTITLE_CLASS}>
                  {resolveMailboxActorLabel(
                    chatActors.fromActorId,
                    displayNameByActorId,
                    normalizedHumanActorId
                  )}{" "}
                  →{" "}
                  {resolveMailboxActorLabel(
                    chatActors.toActorId,
                    displayNameByActorId,
                    normalizedHumanActorId
                  )}
                </div>
                <div className={MAILBOX_CHAT_STATUS_CLASS}>
                  auto_follow={chatStickToBottom ? "on" : "off"}
                </div>
              </div>
              <ToolbarRow className="flex-none justify-end">
                <ActionButton
                  tone="secondary"
                  size="md"
                  onClick={() => {
                    void onAcceptVisibleMessages?.(acceptVisibleMessages);
                  }}
                  disabled={
                    busy !== null ||
                    acceptVisibleMessages.length === 0 ||
                    !onAcceptVisibleMessages
                  }
                  title="Accept pending inbox work visible in this conversation"
                >
                  Accept visible pending
                </ActionButton>
                <ActionButton
                  tone="secondary"
                  size="md"
                  className={MAILBOX_CHAT_JUMP_BUTTON_CLASS}
                  onClick={onJumpToBottom}
                  disabled={conversationMessages.length === 0}
                  title="Jump to latest message"
                >
                  <i className="bi bi-chevron-down" aria-hidden="true" />
                  <span>Jump to bottom</span>
                </ActionButton>
              </ToolbarRow>
            </ToolbarRow>

            <ul
              className={MAILBOX_MESSAGE_LIST_CLASS}
              ref={chatMessagesRef as React.Ref<HTMLUListElement>}
              onScroll={() => onConversationScroll()}
            >
              {conversationRows.map((row) => {
                return (
                  <li
                    key={row.message.message_id}
                    className={`${MAILBOX_MESSAGE_ITEM_CLASS} ${row.isOutgoing ? "items-end" : "items-start"}`}
                  >
                    <div className={row.isOutgoing ? MAILBOX_MESSAGE_BUBBLE_OUTGOING_CLASS : MAILBOX_MESSAGE_BUBBLE_INCOMING_CLASS}>
                      <div className={`${MAILBOX_MESSAGE_HEAD_CLASS} ${row.isOutgoing ? "justify-end text-right" : "justify-start text-left"}`}>
                        <span className="font-bold">
                          {row.fromLabel}
                        </span>
                        <span className="opacity-60">{" → "}</span>
                        <span className="font-bold">
                          {row.toLabel}
                        </span>
                        <span className="opacity-40">{" · "}</span>
                        <span className="opacity-60">{formatTs(row.message.created_at)}</span>
                      </div>
                      <div className={MAILBOX_MESSAGE_BODY_CLASS}>
                        {row.htmlPayload !== null ? (
                          <div
                            onClick={(event) => {
                              const actorId = resolveMentionActorIdFromEventTarget(event.target);
                              if (actorId) {
                                (onOpenMemberProfile ?? onSelectMember)(actorId);
                              }
                            }}
                            dangerouslySetInnerHTML={{
                              __html: row.htmlPayload,
                            }}
                          />
                        ) : (
                          <pre className={MAILBOX_MESSAGE_PRE_CLASS}>{row.payload}</pre>
                        )}
                      </div>
                      {row.canAccept && (
                        <div className={`${MAILBOX_MESSAGE_ACTIONS_CLASS} ${row.isOutgoing ? "justify-end" : "justify-start"}`}>
                          <ActionButton
                            tone="secondary"
                            size="md"
                            onClick={() => {
                              void onAcceptMessage(row.message);
                            }}
                            disabled={busy !== null}
                          >
                            Accept
                          </ActionButton>
                        </div>
                      )}
                    </div>
                  </li>
                );
              })}
              {conversationMessages.length === 0 && (
                <li className={MAILBOX_CONVERSATION_EMPTY_CLASS}>
                  No conversation records yet for this pair.
                </li>
              )}
            </ul>

            <div className={MAILBOX_COMPOSER_CLASS}>
              <textarea
                className={MAILBOX_COMPOSER_TEXTAREA_CLASS}
                rows={3}
                placeholder="Type a message to selected agent"
                value={chatDraft}
                onChange={(event) => onChatDraftChange(event.target.value)}
                onKeyDown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                    event.preventDefault();
                    void onSendChatMessage();
                  }
                }}
              />
              <ToolbarRow className="justify-end">
                <ActionButton
                  tone="primary"
                  size="md"
                  onClick={onSendChatMessage}
                  disabled={busy === "send-chat" || !chatDraft.trim()}
                >
                  Send Chat
                </ActionButton>
              </ToolbarRow>
            </div>
          </div>
        </div>
      )}

      {showConversation && developerMode && (
        <InlineNotice tone="info" className={MAILBOX_ADVANCED_HINT_CLASS}>
          Advanced mailbox tools were moved to <strong>Debug -&gt; Mailbox Raw</strong>.
        </InlineNotice>
      )}

      {showAdvancedControls && !developerMode && (
        <InlineNotice tone="info" className={`${MAILBOX_ADVANCED_HINT_CLASS} mt-3`}>
          Enable Developer Mode in Admin to access raw mailbox tools.
        </InlineNotice>
      )}

      {showDeveloperMailboxTools && (
        <InsetSurface className={MAILBOX_ADVANCED_ROOT_CLASS}>
          <h4 className={MAILBOX_ADVANCED_TITLE_CLASS}>Advanced mailbox controls</h4>
          {advancedControls}
        </InsetSurface>
      )}
    </SurfaceCard>
  );
}

export const TeamMailboxPanel = React.memo(TeamMailboxPanelImpl);
TeamMailboxPanel.displayName = "TeamMailboxPanel";
