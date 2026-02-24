import React from "react";
import {
  TeamConversationMessageRecord,
  TeamMainTaskRecord,
} from "../api";
import {
  TEAM_PANEL_CARD_CLASS,
  TEAM_PANEL_GHOST_BUTTON_CLASS,
  TEAM_PANEL_INPUT_CLASS,
  TEAM_PANEL_PRE_CLASS,
  TEAM_PANEL_PRIMARY_BUTTON_CLASS,
  TEAM_PANEL_REFRESH_BUTTON_CLASS,
  TEAM_PANEL_SECONDARY_BUTTON_CLASS,
  TEAM_PANEL_TEXTAREA_CLASS,
  TEAM_PANEL_TITLE_CLASS,
  TEAM_PANEL_TOOLBAR_ACTIONS_CLASS,
  TEAM_PANEL_TOOLBAR_CLASS,
} from "../ui/tailwind_classes";

export type TeamMainTaskConversationMode = "to_leader" | "to_member" | "group_chat";

type TeamMemberOption = {
  member_id: string;
  role: string;
};

type TeamMainTaskPanelProps = {
  tasks: TeamMainTaskRecord[];
  tasksLoading: boolean;
  selectedMainTaskId: string;
  onSelectedMainTaskIdChange: (value: string) => void;
  onRefreshTasks: () => Promise<void> | void;
  newTaskTitle: string;
  onNewTaskTitleChange: (value: string) => void;
  newTaskTopic: string;
  onNewTaskTopicChange: (value: string) => void;
  newTaskConversationMode: TeamMainTaskConversationMode;
  onNewTaskConversationModeChange: (value: TeamMainTaskConversationMode) => void;
  onCreateTask: () => Promise<void> | void;
  messageRoute: TeamMainTaskConversationMode;
  onMessageRouteChange: (value: TeamMainTaskConversationMode) => void;
  messageTargetMemberId: string;
  onMessageTargetMemberIdChange: (value: string) => void;
  messageDraft: string;
  onMessageDraftChange: (value: string) => void;
  onSendMessage: () => Promise<void> | void;
  onRefreshMessages: () => Promise<void> | void;
  messages: TeamConversationMessageRecord[];
  messagesLoading: boolean;
  busy: string | null;
  memberOptions: TeamMemberOption[];
  formatTs: (ts?: number | null) => string;
  toPrettyJson: (value: unknown) => string;
};

const MAIN_TASK_MODE_OPTIONS: Array<{
  value: TeamMainTaskConversationMode;
  label: string;
}> = [
  { value: "to_leader", label: "to_leader" },
  { value: "to_member", label: "to_member" },
  { value: "group_chat", label: "group_chat" },
];

function resolveMessageText(
  message: TeamConversationMessageRecord,
  toPrettyJson: (value: unknown) => string
): string {
  if (
    typeof message.payload === "object" &&
    message.payload !== null &&
    "type" in message.payload &&
    (message.payload as { type?: unknown }).type === "chat_message" &&
    "text" in message.payload
  ) {
    return String((message.payload as { text?: unknown }).text ?? "");
  }
  return toPrettyJson(message.payload);
}

export function TeamMainTaskPanel(props: TeamMainTaskPanelProps) {
  const {
    tasks,
    tasksLoading,
    selectedMainTaskId,
    onSelectedMainTaskIdChange,
    onRefreshTasks,
    newTaskTitle,
    onNewTaskTitleChange,
    newTaskTopic,
    onNewTaskTopicChange,
    newTaskConversationMode,
    onNewTaskConversationModeChange,
    onCreateTask,
    messageRoute,
    onMessageRouteChange,
    messageTargetMemberId,
    onMessageTargetMemberIdChange,
    messageDraft,
    onMessageDraftChange,
    onSendMessage,
    onRefreshMessages,
    messages,
    messagesLoading,
    busy,
    memberOptions,
    formatTs,
    toPrettyJson,
  } = props;
  const selectedTask = React.useMemo(
    () => tasks.find((task) => task.id === selectedMainTaskId) ?? null,
    [selectedMainTaskId, tasks]
  );
  const canSendMessage =
    selectedMainTaskId.trim().length > 0 &&
    messageDraft.trim().length > 0 &&
    (messageRoute !== "to_member" || messageTargetMemberId.trim().length > 0) &&
    busy !== "send-main-task-message";

  return (
    <div className={TEAM_PANEL_CARD_CLASS}>
      <div className={TEAM_PANEL_TOOLBAR_CLASS}>
        <h3 className={TEAM_PANEL_TITLE_CLASS}>Human - Leader Conversation</h3>
        <div className={TEAM_PANEL_TOOLBAR_ACTIONS_CLASS}>
          <button
            type="button"
            className={TEAM_PANEL_REFRESH_BUTTON_CLASS}
            onClick={() => {
              void onRefreshTasks();
            }}
            disabled={tasksLoading || busy === "refresh-main-task"}
            title="Refresh conversations"
            aria-label="Refresh conversations"
          >
            <i className="bi bi-arrow-clockwise" aria-hidden="true" />
            <span>Refresh Conversations</span>
          </button>
        </div>
      </div>

      <p className="mb-3 text-sm text-slate-600">
        Use conversation planning to sync with the leader before compiling a run.
      </p>

      <div className="grid gap-2 lg:grid-cols-3">
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="new conversation title"
          value={newTaskTitle}
          onChange={(event) => onNewTaskTitleChange(event.target.value)}
        />
        <input
          className={TEAM_PANEL_INPUT_CLASS}
          placeholder="topic (optional)"
          value={newTaskTopic}
          onChange={(event) => onNewTaskTopicChange(event.target.value)}
        />
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={newTaskConversationMode}
          onChange={(event) =>
            onNewTaskConversationModeChange(
              event.target.value as TeamMainTaskConversationMode
            )
          }
        >
          {MAIN_TASK_MODE_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              create_mode={option.label}
            </option>
          ))}
        </select>
      </div>
      <div className="mt-2 flex flex-wrap items-center gap-2">
        <button
          type="button"
          className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
          onClick={() => {
            void onCreateTask();
          }}
          disabled={busy === "create-main-task"}
        >
          Create Conversation
        </button>
        <span className="mono text-xs text-slate-500">
          creator=user route defaults are enforced by backend contract
        </span>
      </div>

      <div className="mt-3 grid gap-2 lg:grid-cols-2">
        <select
          className={TEAM_PANEL_INPUT_CLASS}
          value={selectedMainTaskId}
          onChange={(event) => onSelectedMainTaskIdChange(event.target.value)}
        >
          <option value="">Select conversation</option>
          {tasks.map((task) => (
            <option key={task.id} value={task.id}>
              {task.title} [{task.status}]
            </option>
          ))}
        </select>
        <button
          type="button"
          className={TEAM_PANEL_SECONDARY_BUTTON_CLASS}
          onClick={() => {
            void onRefreshMessages();
          }}
          disabled={!selectedMainTaskId || messagesLoading || busy === "refresh-main-task-messages"}
        >
          Refresh Messages
        </button>
      </div>

      {selectedTask && (
        <div className="mono mt-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-2 text-xs text-slate-700">
          status={selectedTask.status} created_by=
          {selectedTask.created_by_actor_id}
        </div>
      )}

      <div className="mt-3 rounded-xl border border-slate-200 bg-slate-50/60 p-3">
        <div className="mb-2 grid gap-2 lg:grid-cols-3">
          <select
            className={TEAM_PANEL_INPUT_CLASS}
            value={messageRoute}
            onChange={(event) =>
              onMessageRouteChange(event.target.value as TeamMainTaskConversationMode)
            }
          >
            {MAIN_TASK_MODE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                route={option.label}
              </option>
            ))}
          </select>
          {messageRoute === "to_member" ? (
            <select
              className={TEAM_PANEL_INPUT_CLASS}
              value={messageTargetMemberId}
              onChange={(event) => onMessageTargetMemberIdChange(event.target.value)}
            >
              <option value="">Select member target</option>
              {memberOptions.map((member) => (
                <option key={member.member_id} value={member.member_id}>
                  {member.member_id} ({member.role})
                </option>
              ))}
            </select>
          ) : (
            <div className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-500">
              {messageRoute === "to_leader"
                ? "Target is resolved to leader automatically."
                : "Message goes to group channel with no direct target."}
            </div>
          )}
          <button
            type="button"
            className={TEAM_PANEL_GHOST_BUTTON_CLASS}
            onClick={() => {
              onMessageRouteChange("to_leader");
              onMessageTargetMemberIdChange("");
            }}
          >
            Reset Route
          </button>
        </div>
        <textarea
          className={TEAM_PANEL_TEXTAREA_CLASS}
          rows={3}
          placeholder="Type planning message for leader/teammates"
          value={messageDraft}
          onChange={(event) => onMessageDraftChange(event.target.value)}
          onKeyDown={(event) => {
            if ((event.metaKey || event.ctrlKey) && event.key === "Enter" && canSendMessage) {
              event.preventDefault();
              void onSendMessage();
            }
          }}
        />
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <button
            type="button"
            className={TEAM_PANEL_PRIMARY_BUTTON_CLASS}
            onClick={() => {
              void onSendMessage();
            }}
            disabled={!canSendMessage}
          >
            Send Message
          </button>
          <span className="text-xs text-slate-500">shortcut: Ctrl/Cmd + Enter</span>
        </div>
      </div>

      <ul className="teams-message-list mt-3">
        {messages.map((message) => (
          <li
            key={message.message_id}
            className="rounded-lg border border-slate-200 bg-white px-3 py-2"
          >
            <div className="teams-message-head">
              <span className="mono">#{message.message_id}</span>
              <span>
                {message.from_actor_id} -&gt; {message.to_actor_id ?? "-"}
              </span>
              <span>{message.route}</span>
              <span>{formatTs(message.created_at)}</span>
            </div>
            <pre className={TEAM_PANEL_PRE_CLASS}>{resolveMessageText(message, toPrettyJson)}</pre>
          </li>
        ))}
        {!messagesLoading && messages.length === 0 && (
          <li className="rounded-lg border border-dashed border-slate-300 bg-white px-3 py-2 text-sm text-slate-600">
            No conversation messages yet.
          </li>
        )}
        {messagesLoading && (
          <li className="rounded-lg border border-dashed border-slate-300 bg-white px-3 py-2 text-sm text-slate-600">
            Loading conversation messages...
          </li>
        )}
      </ul>
    </div>
  );
}
