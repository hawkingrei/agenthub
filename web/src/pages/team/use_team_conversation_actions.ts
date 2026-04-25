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

function readBootstrapKind(context: unknown): string {
  if (!context || typeof context !== "object" || Array.isArray(context)) {
    return "";
  }
  const bootstrapKind = (context as { bootstrap_kind?: unknown }).bootstrap_kind;
  return typeof bootstrapKind === "string" ? bootstrapKind.trim() : "";
}

const TEAM_CONVERSATION_MESSAGE_LIMIT = 60;
const TEAM_CONVERSATION_MAILBOX_LIMIT = 40;
const TEAM_CONVERSATION_OPTIMISTIC_MESSAGE_BASE_ID = Number.MAX_SAFE_INTEGER - 10_000;
const TEAM_CONVERSATION_DETAIL_REFRESH_COOLDOWN_MS = 30_000;

function conversationDetailKey(teamId: string, taskId: string): string {
  return `${teamId}:${taskId}`;
}

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
  const conversationDetailFetchAtRef = useRef<Map<string, number>>(new Map());
  const conversationDetailRunIdRef = useRef<Map<string, string | null>>(new Map());
  const selectedTeamScopeRef = useRef("");
  const taskMessageScopeRef = useRef<{ teamId: string; taskId: string }>({
    teamId: "",
    taskId: "",
  });

  const clearConversationCollections = useCallback(() => {
    setTaskMessages((current) => (current.length === 0 ? current : []));
    setConversationMailboxMessages((current) => (current.length === 0 ? current : []));
  }, [setConversationMailboxMessages, setTaskMessages]);
  const replaceConversationMailboxMessages = useCallback(
    (messages: TeamActorMessageRecord[]) => {
      startTransition(() => {
        setConversationMailboxMessages((current) => {
          if (messages.length === 0) {
            return current.length === 0 ? current : [];
          }
          return messages;
        });
      });
    },
    [setConversationMailboxMessages]
  );
  const clearConversationMailboxMessages = useCallback(() => {
    replaceConversationMailboxMessages([]);
  }, [replaceConversationMailboxMessages]);
  const selectedConversationBootstrapKind = readBootstrapKind(selectedConversation?.context);

  useEffect(() => {
    const normalizedTeamId = selectedTeamId?.trim() ?? "";
    if (selectedTeamScopeRef.current !== normalizedTeamId) {
      conversationDetailFetchAtRef.current.clear();
      conversationDetailRunIdRef.current.clear();
      selectedTeamScopeRef.current = normalizedTeamId;
    }
  }, [selectedTeamId]);

  useEffect(() => {
    const teamId = selectedTeamId?.trim() ?? "";
    const taskId = (selectedConversation?.id ?? "").trim();
    if (!teamId || !taskId) {
      return;
    }
    const detailKey = conversationDetailKey(teamId, taskId);
    const latestRunId = selectedConversationLatestRun?.id?.trim() || null;
    if (latestRunId) {
      conversationDetailRunIdRef.current.set(detailKey, latestRunId);
      return;
    }
    if (selectedConversationBootstrapKind !== "shared_thread") {
      conversationDetailRunIdRef.current.set(detailKey, null);
    }
  }, [
    selectedConversation?.id,
    selectedConversationBootstrapKind,
    selectedConversationLatestRun,
    selectedTeamId,
  ]);

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
      clearConversationCollections();
    }
    if (!selectedTeamId || !selectedConversation?.id) {
      setTaskMessagesLoading(false);
    }
  }, [
    selectedConversation?.id,
    selectedTeamId,
    setConversationMailboxMessages,
    clearConversationCollections,
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
        clearConversationCollections();
        return;
      }
      setTaskMessagesLoading(true);
      try {
        const selectedConversationId = (selectedConversation?.id ?? "").trim();
        const isSharedThreadTask =
          readBootstrapKind(selectedConversation?.context) === "shared_thread";
        const detailKey = conversationDetailKey(teamId, taskId);
        const cachedConversationRunId =
          conversationDetailRunIdRef.current.get(detailKey);
        const lastDetailFetchAt =
          conversationDetailFetchAtRef.current.get(detailKey) ?? null;
        const now = Date.now();
        const shouldFetchConversationDetail =
          selectedConversationId === taskId &&
          isSharedThreadTask &&
          selectedConversationRunId.length === 0 &&
          cachedConversationRunId === undefined &&
          (lastDetailFetchAt === null ||
            now - lastDetailFetchAt >= TEAM_CONVERSATION_DETAIL_REFRESH_COOLDOWN_MS);
        if (shouldFetchConversationDetail) {
          conversationDetailFetchAtRef.current.set(detailKey, now);
        }
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
        const fetchedConversationRunId =
          taskDetail?.latest_run?.id?.trim() || null;
        if (taskDetail) {
          conversationDetailRunIdRef.current.set(detailKey, fetchedConversationRunId);
        }
        const conversationRunId =
          selectedConversationRunId ||
          fetchedConversationRunId ||
          cachedConversationRunId ||
          "";
        if (conversationRunId) {
          const conversationSnapshot = await api.getTeamRunSnapshot(token, conversationRunId, {
            event_limit: 1,
            message_limit: TEAM_CONVERSATION_MAILBOX_LIMIT,
          });
          if (!isCurrentRequest()) {
            return;
          }
          replaceConversationMailboxMessages(conversationSnapshot.mailbox.recent_messages);
        } else {
          clearConversationMailboxMessages();
        }
      } catch (err) {
        if (!isCurrentRequest()) {
          return;
        }
        setError(parseErrorMessage(err));
        clearConversationMailboxMessages();
      } finally {
        if (isCurrentRequest()) {
          setTaskMessagesLoading(false);
        }
      }
    },
    [
      clearConversationCollections,
      clearConversationMailboxMessages,
      selectedConversation,
      selectedConversationLatestRun,
      selectedTeamId,
      setError,
      setTaskMessages,
      setTaskMessagesLoading,
      replaceConversationMailboxMessages,
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
    clearConversationCollections();
    return {
      task: detail.task,
      latestRunId: detail.latest_run?.id?.trim() || null,
      conversationId: detail.conversation.id?.trim() || null,
    };
  }, [
    clearConversationCollections,
    resolveConversationForMessage,
    selectedTeamId,
    setSharedConversation,
    setSharedConversationLatestRun,
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
              const fallbackConversationId =
                conversation.conversationId?.trim() ||
                prev.find((message) => message.conversation_id.trim().length > 0)
                  ?.conversation_id ||
                taskId;
              const nextMessages = sortConversationMessages([
                ...prev,
                buildOptimisticConversationMessage({
                  messageId: optimisticMessageId!,
                  taskId,
                  conversationId: fallbackConversationId,
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
