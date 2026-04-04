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
  MAILBOX_META_CLASS,
  MAILBOX_SHELL_CLASS,
  MAILBOX_MEMBER_LIST_CLASS,
  MAILBOX_PANEL_CLASS,
  MAILBOX_CHAT_JUMP_BUTTON_CLASS,
  MAILBOX_MESSAGE_LIST_CLASS,
  MAILBOX_MESSAGE_ITEM_CLASS,
  MAILBOX_CONVERSATION_EMPTY_CLASS,
  MAILBOX_MESSAGE_HEAD_CLASS,
  MAILBOX_ADVANCED_GRID_CLASS,
  MAILBOX_ADVANCED_PANEL_CLASS,
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

const MAILBOX_SECTION_TITLE_CLASS = "text-[10px] font-bold uppercase tracking-widest text-notion-text-muted px-1";
const MAILBOX_CHECKBOX_LABEL_CLASS = "checkbox inline-flex items-center gap-2 text-[13px] text-notion-text font-medium cursor-pointer";
const MAILBOX_ADVANCED_HINT_CLASS =
  "mt-4 rounded-md border border-state-warning-border bg-state-warning-bg px-4 py-3 text-[13px] text-state-warning-text italic";
const MAILBOX_ADVANCED_ROOT_CLASS =
  "teams-message-advanced mt-6 rounded-xl border border-notion-border bg-notion-sidebar/10 p-4 sm:p-6";

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

  const actorButtonClassName = (isActive: boolean) =>
    `${TEAM_LIST_ITEM_BASE_CLASS} ${
      isActive ? "ring-1 ring-notion-accent/30 border-notion-accent/30 bg-notion-hover shadow-md" : ""
    }`;

  const advancedControls = (
    <div className={MAILBOX_ADVANCED_GRID_CLASS}>
      <div className={MAILBOX_ADVANCED_PANEL_CLASS}>
        <h4 className="text-[13px] font-bold text-notion-text uppercase tracking-tight">Send Message (JSON)</h4>
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
        <div className="flex flex-col gap-2">
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
        <h4 className="text-[13px] font-bold text-notion-text uppercase tracking-tight">Inbox Query (read-only)</h4>
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
          disabled={busy !== null}
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
    <div className={`${TEAM_PANEL_CARD_CLASS} p-4`}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Mailbox</h3>
      </div>

      {showConversation && snapshot && (
        <div className={MAILBOX_META_CLASS}>
          <span>
            <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Pending:</strong>
            <span className="font-bold">{snapshot.mailbox.pending}</span>
          </span>
          <span>
            <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Delivered:</strong>
            <span className="font-bold">{snapshot.mailbox.delivered}</span>
          </span>
          <span>
            <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Dead Letter:</strong>
            <span className="font-bold">{snapshot.mailbox.dead_letter}</span>
          </span>
          <span>
            <strong className="text-notion-text-muted font-bold uppercase text-[10px] tracking-widest mr-2">Recent Messages:</strong>
            <span className="font-bold">{snapshot.mailbox.recent_messages.length}</span>
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
                  className={actorButtonClassName(selectedMemberId === member.member_id)}
                  onClick={() => onSelectMember(member.member_id)}
                >
                  <div className="flex w-full items-center justify-between gap-2">
                    <span className={`${TEAM_LIST_ITEM_TITLE_CLASS} font-bold`}>
                      {resolveMailboxActorLabel(
                        member.member_id,
                        displayNameByActorId,
                        normalizedHumanActorId
                      )}{" "}
                      ({member.role})
                    </span>
                    {!isHuman && (
                      <StatusBadge
                        label={member.status}
                        tone={resolveTeamRunStatusTone(member.status)}
                        className="team-status"
                        title={`member status: ${member.status}`}
                      />
                    )}
                  </div>
                  <div className="flex w-full items-center justify-between gap-2 mt-0.5">
                    <span className={TEAM_LIST_ITEM_META_CLASS}>
                      {isHuman ? "human actor" : `pending=${member.pending_inbox_count}`}
                    </span>
                    {unread > 0 && (
                      <span className="shrink-0 rounded-sm bg-state-warning-bg border border-state-warning-border px-1.5 py-0.5 text-[10px] font-bold text-state-warning-text">
                        unread={unread}
                      </span>
                    )}
                  </div>
                </button>
              );
            })}
            {mailboxActors.length === 0 && (
              <p className={`${TEAM_MUTED_TEXT_CLASS} px-2`}>No members available.</p>
            )}
          </div>

          <div className={MAILBOX_PANEL_CLASS}>
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-notion-border pb-3">
              <div className="min-w-0 flex-1">
                <span className="text-[10px] font-bold uppercase tracking-widest text-notion-text-muted">Conversation</span>
                <div className="truncate text-[14px] font-bold text-notion-text mt-0.5">
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
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
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
                </button>
                <button
                  type="button"
                  className={MAILBOX_CHAT_JUMP_BUTTON_CLASS}
                  onClick={onJumpToBottom}
                  disabled={conversationMessages.length === 0}
                  title="Jump to latest message"
                >
                  <i className="bi bi-chevron-down" aria-hidden="true" />
                  <span>Jump to bottom</span>
                </button>
              </div>
            </div>

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
                    className={MAILBOX_MESSAGE_ITEM_CLASS}
                  >
                    <div className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-sm text-[10px] font-bold uppercase tracking-tight shadow-sm mt-1 ${isOutgoing ? "bg-notion-accent text-white" : "bg-notion-hover text-notion-text-muted"}`}>
                      {isOutgoing ? "O" : "I"}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className={MAILBOX_MESSAGE_HEAD_CLASS}>
                        <span className="text-notion-text font-bold">
                          {resolveMailboxActorLabel(
                            message.from_actor_id,
                            displayNameByActorId,
                            normalizedHumanActorId
                          )}
                        </span>
                        <span>{" → "}</span>
                        <span className="text-notion-text font-bold">
                          {resolveMailboxActorLabel(
                            message.to_actor_id,
                            displayNameByActorId,
                            normalizedHumanActorId
                          )}
                        </span>
                        <span>{" · "}</span>
                        <span>{formatTs(message.created_at)}</span>
                        <span className="ml-auto mono opacity-60">#{message.message_id}</span>
                      </div>
                      <div className="text-[14px] leading-relaxed text-notion-text mt-1">
                        {chatText !== null ? (
                          <div
                            dangerouslySetInnerHTML={{
                              __html: renderPlainTextWithMentions(payload, displayNameByActorId),
                            }}
                          />
                        ) : (
                          <pre className="mono whitespace-pre-wrap text-[12px] bg-notion-sidebar/30 p-2 rounded border border-notion-border/50">{payload}</pre>
                        )}
                      </div>
                      {isMessageAcceptableForInbox(
                        message,
                        chatActors.inboxActorId,
                        normalizedHumanActorId
                      ) && (
                        <div className="mt-3">
                          <button
                            onClick={() => {
                              void onAcceptMessage(message);
                            }}
                            disabled={busy !== null}
                            className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                          >
                            Accept
                          </button>
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

            <div className="mt-2 flex flex-col gap-2">
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
              <div className="flex justify-end">
                <button
                  className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
                  onClick={onSendChatMessage}
                  disabled={busy === "send-chat" || !chatDraft.trim()}
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
          <h4 className="text-sm font-bold text-notion-text uppercase tracking-widest mb-4">Advanced mailbox controls</h4>
          {advancedControls}
        </div>
      )}
    </div>
  );
}
