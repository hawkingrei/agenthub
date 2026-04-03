import React from "react";
import { TeamActorMessageRecord, TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  isHumanMailboxActor,
  resolveDisplayName,
  renderPlainTextWithMentions,
  resolveVisibleTeamPayloadText,
} from "./team/mailbox_helpers";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_LIST_ITEM_BASE_CLASS,
  TEAM_LIST_ITEM_META_CLASS,
  TEAM_LIST_ITEM_TITLE_CLASS,
  TEAM_MUTED_TEXT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
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

type TeamMailboxPanelProps = {
  developerMode: boolean;
  mode?: "full" | "advanced_only";
  snapshot: TeamRunSnapshotRecord | null;
  humanActorId?: string;
  displayNameByActorId?: Record<string, string>;
  selectedMemberId: string;
  unreadByMemberId: Record<string, number>;
  onSelectMember: (memberId: string) => void;
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

const MAILBOX_META_CLASS =
  "mb-3 grid min-w-0 gap-2 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3 text-ui-sm text-ui-text-secondary sm:grid-cols-2 xl:grid-cols-4";
const MAILBOX_SHELL_CLASS =
  "teams-chat-shell grid min-w-0 gap-3 lg:grid-cols-[minmax(220px,280px)_minmax(0,1fr)]";
const MAILBOX_MEMBER_LIST_CLASS =
  "teams-chat-members flex max-h-[220px] min-w-0 flex-col gap-2 overflow-auto rounded-[18px] border border-ui-border bg-ui-surface-soft/60 p-3 lg:max-h-[560px]";
const MAILBOX_PANEL_CLASS =
  "teams-chat-panel flex min-w-0 flex-col gap-3 rounded-[18px] border border-ui-border bg-ui-surface-soft/60 p-3";
const MAILBOX_MEMBER_BUTTON_BASE_CLASS = `${TEAM_LIST_ITEM_BASE_CLASS}`;
const MAILBOX_MEMBER_BUTTON_ACTIVE_CLASS =
  `${MAILBOX_MEMBER_BUTTON_BASE_CLASS} border-ui-border-strong bg-ui-surface-soft ring-1 ring-ui-border`;
const MAILBOX_MEMBER_BUTTON_IDLE_CLASS =
  `${MAILBOX_MEMBER_BUTTON_BASE_CLASS} hover:border-ui-border-strong`;
const MAILBOX_UNREAD_ACTIVE_CLASS =
  "teams-member-unread mono inline-flex items-center rounded-full border border-state-warning-border bg-state-warning-bg px-2 py-0.5 text-ui-xs text-[color:var(--status-warning-ink)]";
const MAILBOX_UNREAD_MUTED_CLASS =
  "teams-member-unread mono inline-flex items-center rounded-full border border-ui-border bg-ui-surface px-2 py-0.5 text-ui-xs text-ui-text-muted";
const MAILBOX_HEAD_CLASS = "teams-chat-head flex flex-wrap items-center justify-between gap-2";
const MAILBOX_HEAD_META_CLASS =
  "min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-ui-sm text-ui-text-secondary";
const MAILBOX_CHAT_JUMP_BUTTON_CLASS =
  "ghost teams-chat-jump-bottom ml-auto inline-flex items-center rounded-lg border border-ui-border-strong bg-ui-surface px-3 py-2 text-ui-sm font-medium text-ui-text-secondary shadow-sm transition hover:border-ui-border-emphasis hover:bg-ui-surface-soft disabled:cursor-not-allowed disabled:opacity-60";
const MAILBOX_MESSAGE_LIST_CLASS =
  "teams-chat-messages m-0 flex max-h-[420px] list-none flex-col gap-2 overflow-auto p-0";
const MAILBOX_MESSAGE_BUBBLE_BASE_CLASS =
  "teams-chat-bubble rounded-[14px] border px-2.5 py-2.5";
const MAILBOX_MESSAGE_BUBBLE_OUTGOING_CLASS =
  `${MAILBOX_MESSAGE_BUBBLE_BASE_CLASS} border-[rgba(59,130,246,0.18)] bg-[linear-gradient(180deg,rgba(239,246,255,0.95),rgba(248,250,252,0.92))]`;
const MAILBOX_MESSAGE_BUBBLE_INCOMING_CLASS =
  `${MAILBOX_MESSAGE_BUBBLE_BASE_CLASS} border-[rgba(16,185,129,0.14)] bg-[linear-gradient(180deg,rgba(240,253,250,0.94),rgba(248,250,252,0.92))]`;
const MAILBOX_MESSAGE_HEAD_CLASS =
  "teams-message-head mb-1 flex flex-wrap items-center gap-2 text-ui-xs text-ui-text-muted";
const MAILBOX_CONVERSATION_EMPTY_CLASS =
  `teams-chat-empty rounded-xl border border-dashed border-ui-border-strong bg-ui-surface px-3 py-3 ${TEAM_MUTED_TEXT_CLASS}`;
const MAILBOX_ADVANCED_GRID_CLASS = "teams-message-grid grid min-w-0 gap-3 lg:grid-cols-2";
const MAILBOX_ADVANCED_PANEL_CLASS =
  "teams-message-panel flex min-w-0 flex-col gap-2 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3";
const MAILBOX_SECTION_TITLE_CLASS = "text-ui-sm font-semibold text-ui-text-primary";
const MAILBOX_CHECKBOX_LABEL_CLASS = "checkbox inline-flex items-center gap-2 text-ui-sm text-ui-text-secondary";
const MAILBOX_ADVANCED_HINT_CLASS =
  "mt-3 rounded-lg border border-state-warning-border bg-state-warning-bg px-3 py-2 text-ui-sm text-state-warning-text";
const MAILBOX_ADVANCED_ROOT_CLASS =
  "teams-message-advanced mt-3 rounded-xl border border-ui-border bg-ui-surface-soft/70 p-3";

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

export function TeamMailboxPanel(props: TeamMailboxPanelProps) {
  const {
    mode = "full",
    developerMode,
    snapshot,
    humanActorId = "",
    displayNameByActorId = {},
    selectedMemberId,
    unreadByMemberId,
    onSelectMember,
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
  const acceptVisibleMessages = conversationMessages.filter((message) =>
    isMessageAcceptableForInbox(message, chatActors.inboxActorId, normalizedHumanActorId)
  );
  const humanPendingCount =
    snapshot?.mailbox.recent_messages.filter(
      (message) =>
        message.to_actor_id === normalizedHumanActorId &&
        message.status === "pending"
    ).length ?? 0;
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
            status: "human",
          },
        ]
      : []),
  ];
  const advancedControls = (
    <div className={MAILBOX_ADVANCED_GRID_CLASS}>
      <div className={MAILBOX_ADVANCED_PANEL_CLASS}>
        <h4>Send Message (JSON)</h4>
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
        <div className="form-row">
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
          <button
            type="button"
            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
            onClick={onApplyMessageTemplate}
          >
            Apply Template
          </button>
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
        <button
          className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
          onClick={onSendMessage}
          disabled={busy === "send-message"}
        >
          Send Message
        </button>
      </div>

      <div className={MAILBOX_ADVANCED_PANEL_CLASS}>
        <h4>Inbox Query (read-only)</h4>
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
        <button
          className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
          onClick={onRefreshInbox}
          disabled={busy === "refresh-inbox"}
          title="Refresh read-only inbox"
          aria-label="Refresh read-only inbox"
        >
          <i className="bi bi-arrow-clockwise" aria-hidden="true" />
          <span>Refresh Query</span>
        </button>
      </div>
    </div>
  );

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Mailbox</h3>
      </div>

      {showConversation && snapshot && (
        <div className={MAILBOX_META_CLASS}>
          <span>
            <strong>Pending:</strong> {snapshot.mailbox.pending}
          </span>
          <span>
            <strong>Delivered:</strong> {snapshot.mailbox.delivered}
          </span>
          <span>
            <strong>Dead Letter:</strong> {snapshot.mailbox.dead_letter}
          </span>
          <span>
            <strong>Recent Messages:</strong> {snapshot.mailbox.recent_messages.length}
          </span>
        </div>
      )}

      {showConversation && (
        <div className={MAILBOX_SHELL_CLASS}>
          <div className={MAILBOX_MEMBER_LIST_CLASS}>
            <h4 className={MAILBOX_SECTION_TITLE_CLASS}>Agents</h4>
            {mailboxActors.map((member) => {
              const unread = unreadByMemberId[member.member_id] ?? 0;
              const isHuman = member.role === "human";
              return (
                <button
                  key={member.member_id}
                  className={
                    selectedMemberId === member.member_id
                      ? MAILBOX_MEMBER_BUTTON_ACTIVE_CLASS
                      : MAILBOX_MEMBER_BUTTON_IDLE_CLASS
                  }
                  onClick={() => onSelectMember(member.member_id)}
                >
                  <span className={TEAM_LIST_ITEM_TITLE_CLASS}>
                    {resolveMailboxActorLabel(
                      member.member_id,
                      displayNameByActorId,
                      normalizedHumanActorId
                    )}{" "}
                    ({member.role})
                  </span>
                  {isHuman ? (
                    <span className={TEAM_LIST_ITEM_META_CLASS}>human actor</span>
                  ) : (
                    <StatusBadge
                      label={member.status}
                      tone={resolveTeamRunStatusTone(member.status)}
                      className="team-status"
                      title={`member status: ${member.status}`}
                    />
                  )}
                  <span className={TEAM_LIST_ITEM_META_CLASS}>pending={member.pending_inbox_count}</span>
                  <span className={unread > 0 ? MAILBOX_UNREAD_ACTIVE_CLASS : MAILBOX_UNREAD_MUTED_CLASS}>
                    unread={unread}
                  </span>
                </button>
              );
            })}
            {mailboxActors.length === 0 && (
              <p className={TEAM_MUTED_TEXT_CLASS}>No members available.</p>
            )}
          </div>

          <div className={MAILBOX_PANEL_CLASS}>
            <div className={MAILBOX_HEAD_CLASS}>
              <div className={MAILBOX_HEAD_META_CLASS}>
                <strong>
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
                </strong>
              </div>
              <div className="flex flex-wrap items-center justify-end gap-2">
                {developerMode && (
                  <>
                    <div className={`mono ${MAILBOX_HEAD_META_CLASS}`}>
                      inbox_actor_id={chatActors.inboxActorId || "-"}
                    </div>
                    <div className={`mono ${MAILBOX_HEAD_META_CLASS}`}>
                      auto_follow={chatStickToBottom ? "on" : "off"}
                    </div>
                  </>
                )}
                <button
                  type="button"
                  className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                  onClick={() => {
                    void onAcceptVisibleMessages?.(acceptVisibleMessages);
                  }}
                  disabled={
                    busy === "accept-visible" ||
                    acceptVisibleMessages.length === 0 ||
                    !onAcceptVisibleMessages
                  }
                  title="Accept pending inbox work visible in this conversation"
                >
                  Accept visible pending
                </button>
                <button
                  type="button"
                  className={MAILBOX_CHAT_JUMP_BUTTON_CLASS}
                  onClick={onJumpToBottom}
                  disabled={conversationMessages.length === 0}
                  title="Jump to latest message"
                >
                  Jump to bottom
                </button>
              </div>
            </div>
            <p className={`mb-3 ${TEAM_MUTED_TEXT_CLASS}`}>
              Refresh is read-only. Use Accept to consume pending inbox work for{" "}
              <code>{chatActors.inboxActorId || "-"}</code>.
            </p>
            <ul
              className={MAILBOX_MESSAGE_LIST_CLASS}
              ref={chatMessagesRef}
              onScroll={() => onConversationScroll()}
            >
              {conversationMessages.map((message) => {
                const isOutgoing = message.from_actor_id === chatActors.fromActorId;
                const chatText = resolveVisibleTeamPayloadText(message.payload);
                const payload = chatText ?? toPrettyJson(message.payload);
                return (
                  <li
                    key={message.message_id}
                    className={
                      isOutgoing
                        ? MAILBOX_MESSAGE_BUBBLE_OUTGOING_CLASS
                        : MAILBOX_MESSAGE_BUBBLE_INCOMING_CLASS
                    }
                  >
                    <div className={MAILBOX_MESSAGE_HEAD_CLASS}>
                      <span className="mono">#{message.message_id}</span>
                      <span>
                        {resolveMailboxActorLabel(
                          message.from_actor_id,
                          displayNameByActorId,
                          normalizedHumanActorId
                        )}{" "}
                        →{" "}
                        {resolveMailboxActorLabel(
                          message.to_actor_id,
                          displayNameByActorId,
                          normalizedHumanActorId
                        )}
                      </span>
                      <span>{message.status}</span>
                      <span>{formatTs(message.created_at)}</span>
                    </div>
                    {chatText !== null ? (
                      <div
                        className={`${TEAM_PANEL_PRE_CLASS} whitespace-pre-wrap`}
                        dangerouslySetInnerHTML={{
                          __html: renderPlainTextWithMentions(payload, displayNameByActorId),
                        }}
                      />
                    ) : (
                      <pre className={TEAM_PANEL_PRE_CLASS}>{payload}</pre>
                    )}
                    {isMessageAcceptableForInbox(
                      message,
                      chatActors.inboxActorId,
                      normalizedHumanActorId
                    ) && (
                      <div className="flex flex-wrap items-center gap-2">
                        <button
                          onClick={() => {
                            void onAcceptMessage(message);
                          }}
                          disabled={busy === `accept-${message.message_id}`}
                          className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                        >
                          Accept
                        </button>
                      </div>
                    )}
                  </li>
                );
              })}
              {conversationMessages.length === 0 && (
                <li className={MAILBOX_CONVERSATION_EMPTY_CLASS}>
                  No conversation records yet for this pair.
                </li>
              )}
            </ul>
            <div className="teams-chat-compose">
              <textarea
                className={TEAM_PANEL_TEXTAREA_CLASS}
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
              <div className="flex flex-wrap items-center gap-2">
                <button
                  className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
                  onClick={onSendChatMessage}
                  disabled={busy === "send-chat"}
                >
                  Send Chat
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {showConversation && developerMode && (
        <div className={MAILBOX_ADVANCED_HINT_CLASS}>
          Advanced mailbox tools were moved to <strong>Debug -&gt; Mailbox Raw</strong>.
        </div>
      )}

      {showAdvancedControls && !developerMode && (
        <div className={`${MAILBOX_ADVANCED_HINT_CLASS} mt-3`}>
          Enable Developer Mode in Admin to access raw mailbox tools.
        </div>
      )}

      {showDeveloperMailboxTools && (
        <div className={MAILBOX_ADVANCED_ROOT_CLASS}>
          <h4 className={`mb-2 ${MAILBOX_SECTION_TITLE_CLASS}`}>Advanced mailbox controls</h4>
          {advancedControls}
        </div>
      )}
    </div>
  );
}
