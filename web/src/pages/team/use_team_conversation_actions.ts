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
  type TeamTaskRecord,
} from "../../api";
import { buildMailboxChatPayload } from "./mailbox_helpers";
import {
  isSharedThreadTask,
  mergeConversationMessages,
  refreshTeamConversationMailboxAfterSend,
} from "./page_helpers";
import { parseErrorMessage } from "./create_helpers";

const TEAM_CONVERSATION_INITIAL_MESSAGE_LIMIT = 60;
const TEAM_CONVERSATION_EXTENDED_MESSAGE_LIMIT = 200;
const TEAM_CONVERSATION_INITIAL_MAILBOX_LIMIT = 60;
const TEAM_CONVERSATION_EXTENDED_MAILBOX_LIMIT = 200;

type SharedConversationTarget = {
  task: TeamTaskRecord;
  latestRunId: string | null;
};

type UseTeamConversationActionsOptions = {
  token: string;
  selectedTeamId: string | null;
  selectedConversation: TeamTaskRecord | null;
  latestRunForSharedConversation: TeamRunRecord | null;
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
  latestRunForSharedConversation,
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
  const taskMessageScopeRef = useRef<{ teamId: string; taskId: string }>({
    teamId: "",
    taskId: "",
  });

  useEffect(() => {
    taskMessageRequestSeqRef.current += 1;
    taskMessageScopeRef.current = {
      teamId: selectedTeamId?.trim() ?? "",
      taskId: (selectedConversation?.id ?? "").trim(),
    };
    if (!selectedTeamId || !selectedConversation?.id) {
      setTaskMessagesLoading(false);
    }
  }, [selectedConversation?.id, selectedTeamId, setTaskMessagesLoading]);

  const refreshTaskMessages = useCallback(
    async (taskIdOverride?: string) => {
      const teamId = selectedTeamId?.trim() ?? "";
      const taskId = (taskIdOverride ?? selectedConversation?.id ?? "").trim();
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
        const [messages, taskDetail] = await Promise.all([
          api.listTeamTaskMessages(token, teamId, taskId, {
            limit: TEAM_CONVERSATION_INITIAL_MESSAGE_LIMIT,
          }),
          selectedConversation &&
          selectedConversation.id === taskId &&
          isSharedThreadTask(selectedConversation)
            ? api.getTeamTask(token, teamId, taskId)
            : Promise.resolve(null),
        ]);
        if (!isCurrentRequest()) {
          return;
        }
        startTransition(() => {
          setTaskMessages((prev) => mergeConversationMessages(prev, messages));
        });
        const conversationRunId = taskDetail?.latest_run?.id?.trim() ?? "";
        if (conversationRunId) {
          const conversationSnapshot = await api.getTeamRunSnapshot(token, conversationRunId, {
            event_limit: 1,
            message_limit: TEAM_CONVERSATION_INITIAL_MAILBOX_LIMIT,
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
        void (async () => {
          try {
            const extendedMessagesPromise =
              TEAM_CONVERSATION_EXTENDED_MESSAGE_LIMIT >
              TEAM_CONVERSATION_INITIAL_MESSAGE_LIMIT
                ? api.listTeamTaskMessages(token, teamId, taskId, {
                    limit: TEAM_CONVERSATION_EXTENDED_MESSAGE_LIMIT,
                  })
                : Promise.resolve<TeamConversationMessageRecord[]>(messages);
            const extendedMailboxPromise =
              conversationRunId &&
              TEAM_CONVERSATION_EXTENDED_MAILBOX_LIMIT >
                TEAM_CONVERSATION_INITIAL_MAILBOX_LIMIT
                ? api.getTeamRunSnapshot(token, conversationRunId, {
                    event_limit: 1,
                    message_limit: TEAM_CONVERSATION_EXTENDED_MAILBOX_LIMIT,
                  })
                : Promise.resolve(null);
            const [extendedMessages, extendedMailboxSnapshot] = await Promise.all([
              extendedMessagesPromise,
              extendedMailboxPromise,
            ]);
            if (!isCurrentRequest()) {
              return;
            }
            startTransition(() => {
              setTaskMessages((prev) => mergeConversationMessages(prev, extendedMessages));
            });
            if (extendedMailboxSnapshot) {
              startTransition(() => {
                setConversationMailboxMessages(extendedMailboxSnapshot.mailbox.recent_messages);
              });
            }
          } catch {
            // Keep the fast-path payload visible even if the background hydration misses.
          }
        })();
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
      latestRunId: latestRunForSharedConversation?.id?.trim() || null,
    };
  }, [latestRunForSharedConversation, selectedConversation, selectedTeamId]);

  const ensureSharedConversation = useCallback(async (): Promise<SharedConversationTarget | null> => {
    if (!selectedTeamId) {
      return null;
    }
    const existing = resolveConversationForMessage();
    if (existing) {
      return existing;
    }
    const detail = await api.ensureTeamSharedThread(token, selectedTeamId);
    setSharedConversation(detail.task);
    setSharedConversationLatestRun(detail.latest_run ?? null);
    setTaskMessages([]);
    setConversationMailboxMessages([]);
    return {
      task: detail.task,
      latestRunId: detail.latest_run?.id?.trim() || null,
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
      const text = payload.text.trim();
      if (!text) {
        setError("Conversation message is required");
        return;
      }
      setBusy("send-task-message");
      setError(null);
      setWarning(null);
      try {
        const conversation = await ensureSharedConversation();
        const taskId = conversation?.task.id;
        const chatPayload = buildMailboxChatPayload(text, {
          mention_actor_ids: payload.mentionActorIds,
        });
        if (taskId) {
          const message = await api.sendTeamTaskMessage(token, selectedTeamId, taskId, {
            payload: chatPayload,
          });
          setTaskMessages((prev) =>
            mergeConversationMessages(prev, [...prev, message].sort((left, right) => left.message_id - right.message_id))
          );
          await refreshTeamConversationMailboxAfterSend({
            activeRunId: activeRunIdForSelectedTeam ?? conversation?.latestRunId,
            taskId,
            refreshSnapshot,
            refreshEvents,
            refreshTaskMessages,
          });
          setTaskMessageDraft("");
          return;
        }
        setWarning("Unable to initialize shared team thread.");
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
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
