import {
  startTransition,
  useCallback,
  useEffect,
  useRef,
  type Dispatch,
  type SetStateAction,
} from "react";
import {
  api,
  type TeamActorMessageRecord,
  type TeamConversationMessageRecord,
  type TeamRunRecord,
  type TeamTaskDetailResponse,
  type TeamTaskRecord,
} from "../../api";
import { buildMailboxChatPayload } from "./mailbox_helpers";
import {
  mergeConversationMessages,
  refreshTeamConversationMailboxAfterSend,
} from "./page_helpers";
import { parseErrorMessage } from "./create_helpers";
import { uuidV7 } from "../../uuid";

const TEAM_CONVERSATION_MESSAGE_LIMIT = 20;
const TEAM_CONVERSATION_MAILBOX_LIMIT = 20;
const TEAM_CONVERSATION_OPTIMISTIC_MESSAGE_BASE_ID = Number.MAX_SAFE_INTEGER - 10_000;

type SharedConversationTarget = {
  task: TeamTaskRecord;
  latestRunId: string | null;
  conversationId: string | null;
};

type UseTeamConversationActionsOptions = {
  token: string;
  selectedTeamId: string | null;
  selectedConversation: TeamTaskRecord | null;
  selectedConversationLatestRun: TeamRunRecord | null;
  activeRunIdForSelectedTeam: string | null;
  refreshSnapshot: (runId: string) => Promise<unknown>;
  refreshEvents: (runId: string) => Promise<unknown>;
  setBusy: Dispatch<SetStateAction<string | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  setWarning: Dispatch<SetStateAction<string | null>>;
  setSharedConversation: Dispatch<SetStateAction<TeamTaskRecord | null>>;
  setSharedConversationLatestRun: Dispatch<SetStateAction<TeamRunRecord | null>>;
  setTaskMessages: Dispatch<SetStateAction<TeamConversationMessageRecord[]>>;
  setTaskMessagesLoading: Dispatch<SetStateAction<boolean>>;
  setConversationMailboxMessages: Dispatch<SetStateAction<TeamActorMessageRecord[]>>;
  setTaskMessageDraft: Dispatch<SetStateAction<string>>;
};

export function useTeamConversationActions({
  activeRunIdForSelectedTeam,
  refreshEvents,
  refreshSnapshot,
  selectedConversation,
  selectedTeamId,
  selectedConversationLatestRun,
  setBusy,
  setConversationMailboxMessages,
  setError,
  setSharedConversation,
  setSharedConversationLatestRun,
  setTaskMessageDraft,
  setTaskMessages,
  setTaskMessagesLoading,
  setWarning,
  token,
}: UseTeamConversationActionsOptions) {
  const taskMessageRequestSeqRef = useRef(0);
  const optimisticTaskMessageSeqRef = useRef(0);
  const sendTaskMessageInFlightRef = useRef(false);
  const taskMessageScopeRef = useRef<{ teamId: string; taskId: string }>({
    teamId: "",
    taskId: "",
  });

  useEffect(() => {
    const nextScope = {
      teamId: selectedTeamId?.trim() ?? "",
      taskId: (selectedConversation?.id ?? "").trim(),
    };
    const previousScope = taskMessageScopeRef.current;
    taskMessageRequestSeqRef.current += 1;
    taskMessageScopeRef.current = nextScope;
    const conversationScopeChanged =
      previousScope.teamId !== nextScope.teamId || previousScope.taskId !== nextScope.taskId;
    if (conversationScopeChanged) {
      setTaskMessages([]);
      setConversationMailboxMessages([]);
    }
    if (!selectedTeamId || !selectedConversation?.id) {
      setTaskMessagesLoading(false);
    }
  }, [
    selectedConversation?.id,
    selectedTeamId,
    setConversationMailboxMessages,
    setTaskMessages,
    setTaskMessagesLoading,
  ]);

  const refreshTaskMessages = useCallback(
    async (taskIdOverride?: string) => {
      const teamId = selectedTeamId?.trim() ?? "";
      const taskId = (taskIdOverride ?? selectedConversation?.id ?? "").trim();
      const selectedConversationRunId = selectedConversationLatestRun?.id?.trim() ?? "";
      const requestSeq = ++taskMessageRequestSeqRef.current;
      taskMessageScopeRef.current = { teamId, taskId };
      const isCurrentRequest = () =>
        requestSeq === taskMessageRequestSeqRef.current &&
        taskMessageScopeRef.current.teamId === teamId &&
        taskMessageScopeRef.current.taskId === taskId;
      if (!teamId || !taskId) {
        setTaskMessages([]);
        setConversationMailboxMessages([]);
        return;
      }
      setTaskMessagesLoading(true);
      try {
        const shouldFetchConversationDetail =
          Boolean(selectedConversation && selectedConversation.id === taskId) &&
          selectedConversationRunId.length === 0;
        const [messages, taskDetail] = await Promise.all([
          api.listTeamTaskMessages(token, teamId, taskId, {
            limit: TEAM_CONVERSATION_MESSAGE_LIMIT,
          }),
          shouldFetchConversationDetail
            ? api.getTeamTask(token, teamId, taskId)
            : Promise.resolve(null),
        ]);
        if (!isCurrentRequest()) {
          return;
        }
        startTransition(() => {
          setTaskMessages((prev) => mergeConversationMessages(prev, messages));
        });
        const conversationRunId =
          selectedConversationRunId || taskDetail?.latest_run?.id?.trim() || "";
        if (conversationRunId) {
          const conversationSnapshot = await api.getTeamRunSnapshot(token, conversationRunId, {
            event_limit: 1,
            message_limit: TEAM_CONVERSATION_MAILBOX_LIMIT,
          });
          if (!isCurrentRequest()) {
            return;
          }
          startTransition(() => {
            setConversationMailboxMessages(conversationSnapshot.mailbox.recent_messages);
          });
        } else {
          startTransition(() => {
            setConversationMailboxMessages((prev) => (prev.length === 0 ? prev : []));
          });
        }
      } catch (err) {
        if (!isCurrentRequest()) {
          return;
        }
        setError(parseErrorMessage(err));
        startTransition(() => {
          setConversationMailboxMessages((prev) => (prev.length === 0 ? prev : []));
        });
      } finally {
        if (isCurrentRequest()) {
          setTaskMessagesLoading(false);
        }
      }
    },
    [
      selectedConversation,
      selectedConversationLatestRun,
      selectedTeamId,
      setConversationMailboxMessages,
      setError,
      setTaskMessages,
      setTaskMessagesLoading,
      token,
    ]
  );

  const resolveConversationForMessage = useCallback((): SharedConversationTarget | null => {
    if (!selectedTeamId || !selectedConversation) {
      return null;
    }
    return {
      task: selectedConversation,
      latestRunId: selectedConversationLatestRun?.id?.trim() || null,
      conversationId: null,
    };
  }, [selectedConversation, selectedConversationLatestRun, selectedTeamId]);

  const ensureSharedConversation = useCallback(async (): Promise<SharedConversationTarget | null> => {
    if (!selectedTeamId) {
      return null;
    }
    const existing = resolveConversationForMessage();
    if (existing) {
      return existing;
    }
    const detail = await api.ensureTeamSharedThread(token, selectedTeamId);
    applySharedConversationDetail(detail, setSharedConversation, setSharedConversationLatestRun);
    setTaskMessages([]);
    setConversationMailboxMessages([]);
    return {
      task: detail.task,
      latestRunId: detail.latest_run?.id?.trim() || null,
      conversationId: detail.conversation.id?.trim() || null,
    };
  }, [
    resolveConversationForMessage,
    selectedTeamId,
    setConversationMailboxMessages,
    setSharedConversation,
    setSharedConversationLatestRun,
    setTaskMessages,
    token,
  ]);

  const sendTaskMessage = useCallback(
    async (payload: { text: string; mentionActorIds: string[] }) => {
      if (!selectedTeamId) {
        setError("Select a team first");
        return;
      }
      if (sendTaskMessageInFlightRef.current) {
        return;
      }
      const text = payload.text.trim();
      if (!text) {
        setError("Conversation message is required");
        return;
      }
      setTaskMessageDraft("");
      sendTaskMessageInFlightRef.current = true;
      setBusy("send-task-message");
      setError(null);
      setWarning(null);
      let optimisticMessageId: number | null = null;
      const restoreDraft = () => {
        setTaskMessageDraft((current) =>
          current.trim().length > 0 ? `${text}\n${current}` : text
        );
      };
      try {
        const conversation = await ensureSharedConversation();
        const taskId = conversation?.task.id;
        const chatPayload = buildMailboxChatPayload(text, {
          mention_actor_ids: payload.mentionActorIds,
        });
        if (taskId) {
          const idempotencyKey = `team-task-message:${uuidV7()}`;
          optimisticMessageId =
            TEAM_CONVERSATION_OPTIMISTIC_MESSAGE_BASE_ID +
            ++optimisticTaskMessageSeqRef.current;
          setTaskMessages((prev) =>
            {
              const nextMessages = sortConversationMessages([
                ...prev,
                buildOptimisticConversationMessage({
                  messageId: optimisticMessageId!,
                  taskId,
                  conversationId: conversation.conversationId,
                  payload: chatPayload,
                  mentionActorIds: payload.mentionActorIds,
                }),
              ]);
              return mergeConversationMessages(prev, nextMessages);
            }
          );
          const message = await api.sendTeamTaskMessage(token, selectedTeamId, taskId, {
            payload: chatPayload,
            idempotency_key: idempotencyKey,
          });
          setTaskMessages((prev) => {
            const filtered = prev.filter((item) => item.message_id !== optimisticMessageId);
            const nextMessages = sortConversationMessages([...filtered, message]);
            return mergeConversationMessages(filtered, nextMessages);
          });
          await refreshTeamConversationMailboxAfterSend({
            activeRunId: activeRunIdForSelectedTeam ?? conversation?.latestRunId,
            taskId,
            refreshSnapshot,
            refreshEvents,
            refreshTaskMessages,
          });
          return;
        }
        restoreDraft();
        setWarning("Unable to initialize shared team thread.");
      } catch (err) {
        if (optimisticMessageId != null) {
          setTaskMessages((prev) =>
            prev.filter((message) => message.message_id !== optimisticMessageId)
          );
        }
        restoreDraft();
        setError(parseErrorMessage(err));
      } finally {
        sendTaskMessageInFlightRef.current = false;
        setBusy(null);
      }
    },
    [
      activeRunIdForSelectedTeam,
      ensureSharedConversation,
      refreshEvents,
      refreshSnapshot,
      refreshTaskMessages,
      selectedTeamId,
      setBusy,
      setError,
      setTaskMessageDraft,
      setTaskMessages,
      setWarning,
      token,
    ]
  );

  return {
    ensureSharedConversation,
    refreshTaskMessages,
    sendTaskMessage,
  };
}

function applySharedConversationDetail(
  detail: TeamTaskDetailResponse,
  setSharedConversation: Dispatch<SetStateAction<TeamTaskRecord | null>>,
  setSharedConversationLatestRun: Dispatch<SetStateAction<TeamRunRecord | null>>
) {
  setSharedConversation(detail.task);
  setSharedConversationLatestRun(detail.latest_run ?? null);
}

function sortConversationMessages(
  messages: TeamConversationMessageRecord[]
): TeamConversationMessageRecord[] {
  return [...messages].sort((left, right) => {
    if (left.created_at !== right.created_at) {
      return left.created_at - right.created_at;
    }
    if (left.message_id !== right.message_id) {
      return left.message_id - right.message_id;
    }
    return left.from_actor_id.localeCompare(right.from_actor_id);
  });
}

function buildOptimisticConversationMessage({
  messageId,
  taskId,
  conversationId,
  payload,
  mentionActorIds,
}: {
  messageId: number;
  taskId: string;
  conversationId?: string | null;
  payload: ReturnType<typeof buildMailboxChatPayload>;
  mentionActorIds: string[];
}): TeamConversationMessageRecord {
  return {
    message_id: messageId,
    conversation_id: conversationId ?? "",
    task_id: taskId,
    from_actor_id: "user",
    to_actor_id: mentionActorIds.length === 1 ? mentionActorIds[0] ?? null : null,
    route: mentionActorIds.length === 1 ? "to_member" : "group_chat",
    payload,
    created_at: Math.floor(Date.now() / 1000),
  };
}
