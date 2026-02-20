import React from "react";
import { TeamActorMessageRecord, TeamRunSnapshotRecord } from "../api";
import { StatusBadge, resolveTeamRunStatusTone } from "../components/status_badge";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
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
  snapshot: TeamRunSnapshotRecord | null;
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
  onAckMessage: (message: TeamActorMessageRecord) => Promise<void> | void;
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
  "teams-run-meta mb-3 grid gap-2 rounded-xl border border-slate-200 bg-slate-50/70 p-3 text-sm text-slate-700 sm:grid-cols-2 xl:grid-cols-4";
const MAILBOX_MEMBER_LIST_CLASS = "teams-chat-members rounded-xl border border-slate-200 bg-slate-50/60 p-3";
const MAILBOX_PANEL_CLASS = "teams-chat-panel rounded-xl border border-slate-200 bg-slate-50/60 p-3";
const MAILBOX_MEMBER_BUTTON_BASE_CLASS =
  "team-item rounded-lg border bg-white px-3 py-2 text-left transition";
const MAILBOX_MEMBER_BUTTON_ACTIVE_CLASS =
  `${MAILBOX_MEMBER_BUTTON_BASE_CLASS} active border-slate-300 ring-1 ring-slate-200`;
const MAILBOX_MEMBER_BUTTON_IDLE_CLASS =
  `${MAILBOX_MEMBER_BUTTON_BASE_CLASS} border-slate-200 hover:border-slate-300`;
const MAILBOX_UNREAD_ACTIVE_CLASS = "teams-member-unread mono text-amber-700";
const MAILBOX_UNREAD_MUTED_CLASS = "teams-member-unread mono muted text-slate-500";
const MAILBOX_CHAT_JUMP_BUTTON_CLASS =
  "ghost teams-chat-jump-bottom rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm font-medium hover:border-slate-500 disabled:cursor-not-allowed disabled:opacity-60";
const MAILBOX_CONVERSATION_EMPTY_CLASS = "teams-chat-empty muted text-sm text-slate-600";

export function TeamMailboxPanel(props: TeamMailboxPanelProps) {
  const {
    snapshot,
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
    onAckMessage,
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

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Mailbox</h3>
      </div>

      {snapshot && (
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

      <div className="teams-chat-shell">
        <div className={MAILBOX_MEMBER_LIST_CLASS}>
          <h4>Agents</h4>
          {snapshot?.members.map((member) => {
            const unread = unreadByMemberId[member.member_id] ?? 0;
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
                <span className="team-name">
                  {member.member_id} ({member.role})
                </span>
                <StatusBadge
                  label={member.status}
                  tone={resolveTeamRunStatusTone(member.status)}
                  className="team-status"
                  title={`member status: ${member.status}`}
                />
                <span className="team-id mono">pending={member.pending_inbox_count}</span>
                <span className={unread > 0 ? MAILBOX_UNREAD_ACTIVE_CLASS : MAILBOX_UNREAD_MUTED_CLASS}>
                  unread={unread}
                </span>
              </button>
            );
          })}
          {(!snapshot || snapshot.members.length === 0) && (
            <p className="muted text-sm text-slate-600">No members available.</p>
          )}
        </div>

        <div className={MAILBOX_PANEL_CLASS}>
          <div className="teams-chat-head">
            <div>
              <strong>
                {chatActors.fromActorId || "-"} → {chatActors.toActorId || "-"}
              </strong>
            </div>
            <div className="mono">inbox_actor_id={chatActors.inboxActorId || "-"}</div>
            <div className="mono">auto_follow={chatStickToBottom ? "on" : "off"}</div>
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
          <ul
            className="teams-chat-messages"
            ref={chatMessagesRef}
            onScroll={() => onConversationScroll()}
          >
            {conversationMessages.map((message) => {
              const isOutgoing = message.from_actor_id === chatActors.fromActorId;
              const payload =
                typeof message.payload === "object" &&
                message.payload !== null &&
                "type" in message.payload &&
                (message.payload as { type?: unknown }).type === "chat_message" &&
                "text" in message.payload
                  ? String((message.payload as { text?: unknown }).text ?? "")
                  : toPrettyJson(message.payload);
              return (
                <li
                  key={message.message_id}
                  className={isOutgoing ? "teams-chat-bubble outgoing" : "teams-chat-bubble incoming"}
                >
                  <div className="teams-message-head">
                    <span className="mono">#{message.message_id}</span>
                    <span>
                      {message.from_actor_id} → {message.to_actor_id}
                    </span>
                    <span>{message.status}</span>
                    <span>{formatTs(message.created_at)}</span>
                  </div>
                  <pre className="mono">{payload}</pre>
                  {message.status !== "delivered" && (
                    <div className="actions">
                      <button
                        onClick={() => {
                          void onAckMessage(message);
                        }}
                        disabled={busy === `ack-${message.message_id}`}
                        className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
                      >
                        Ack
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
            <div className="actions">
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

      <details className="teams-message-advanced">
        <summary>Advanced mailbox controls</summary>
        <div className="teams-message-grid">
          <div className="teams-message-panel">
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

          <div className="teams-message-panel">
            <h4>Inbox (raw query)</h4>
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
            <label className="checkbox inline-flex items-center gap-2 text-sm text-slate-700">
              <input
                type="checkbox"
                checked={inboxIncludeDelivered}
                onChange={(event) => onInboxIncludeDeliveredChange(event.target.checked)}
              />
              include_delivered
            </label>
            <button
              className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
              onClick={onRefreshInbox}
              disabled={busy === "refresh-inbox"}
            >
              Refresh Inbox
            </button>
          </div>
        </div>
      </details>
    </div>
  );
}
