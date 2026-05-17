import { useCallback, useReducer, useRef } from "react";
import type { TeamActorMessageRecord } from "../../api";
import {
  DEFAULT_TEAM_MAILBOX_STATE,
  reduceTeamMailboxState,
  resolveUpdater,
  type TeamMailboxState,
} from "./state";
import type { MailboxTemplateKey } from "./mailbox_helpers";
import type { SetStateAction } from "react";

export function useTeamMailboxState() {
  const [state, dispatch] = useReducer(reduceTeamMailboxState, DEFAULT_TEAM_MAILBOX_STATE);
  const stateRef = useRef(state);
  stateRef.current = state;

  const patchTeamMailbox = useCallback(
    (patch: Partial<TeamMailboxState>) => {
      dispatch({ type: "patch", patch });
    },
    [dispatch]
  );

  const setMsgFromActorId = useCallback(
    (next: string) => patchTeamMailbox({ msgFromActorId: next }),
    [patchTeamMailbox]
  );

  const setMsgToActorId = useCallback(
    (next: string) => patchTeamMailbox({ msgToActorId: next }),
    [patchTeamMailbox]
  );

  const setMsgChannel = useCallback(
    (next: string) => patchTeamMailbox({ msgChannel: next }),
    [patchTeamMailbox]
  );

  const setMsgTransport = useCallback(
    (next: "local" | "remote") => patchTeamMailbox({ msgTransport: next }),
    [patchTeamMailbox]
  );

  const setMsgRoute = useCallback(
    (next: string) => patchTeamMailbox({ msgRoute: next }),
    [patchTeamMailbox]
  );

  const setMsgTemplate = useCallback(
    (next: MailboxTemplateKey) => patchTeamMailbox({ msgTemplate: next }),
    [patchTeamMailbox]
  );

  const setMsgPayload = useCallback(
    (next: string) => patchTeamMailbox({ msgPayload: next }),
    [patchTeamMailbox]
  );

  const setMsgIdempotencyKey = useCallback(
    (next: string) => patchTeamMailbox({ msgIdempotencyKey: next }),
    [patchTeamMailbox]
  );

  const setChatDraft = useCallback(
    (next: string) => patchTeamMailbox({ chatDraft: next }),
    [patchTeamMailbox]
  );

  const setChatStickToBottom = useCallback(
    (next: SetStateAction<boolean>) =>
      patchTeamMailbox({
        chatStickToBottom: resolveUpdater(stateRef.current.chatStickToBottom, next),
      }),
    [patchTeamMailbox]
  );

  const setChatSeenByConversation = useCallback(
    (next: SetStateAction<Record<string, number>>) =>
      patchTeamMailbox({
        chatSeenByConversation: resolveUpdater(stateRef.current.chatSeenByConversation, next),
      }),
    [patchTeamMailbox]
  );

  const setInboxActorId = useCallback(
    (next: string) => patchTeamMailbox({ inboxActorId: next }),
    [patchTeamMailbox]
  );

  const setInboxLimit = useCallback(
    (next: string) => patchTeamMailbox({ inboxLimit: next }),
    [patchTeamMailbox]
  );

  const setInboxAfterId = useCallback(
    (next: string) => patchTeamMailbox({ inboxAfterId: next }),
    [patchTeamMailbox]
  );

  const setInboxIncludeDelivered = useCallback(
    (next: boolean) => patchTeamMailbox({ inboxIncludeDelivered: next }),
    [patchTeamMailbox]
  );

  const setInbox = useCallback(
    (next: SetStateAction<TeamActorMessageRecord[]>) =>
      patchTeamMailbox({
        inbox: resolveUpdater(stateRef.current.inbox, next),
      }),
    [patchTeamMailbox]
  );

  const setSelectedMemberId = useCallback(
    (next: SetStateAction<string>) =>
      patchTeamMailbox({
        selectedMemberId: resolveUpdater(stateRef.current.selectedMemberId, next),
      }),
    [patchTeamMailbox]
  );

  const markConversationSeen = useCallback(
    (key: string, messageId: number | null) => {
      if (!key || messageId == null) {
        return;
      }
      dispatch({
        type: "mark_conversation_seen",
        key,
        messageId,
      });
    },
    [dispatch]
  );

  const resetChatSeen = useCallback(() => {
    dispatch({ type: "reset_chat_seen" });
  }, [dispatch]);

  return {
    ...state,
    patchTeamMailbox,
    setMsgFromActorId,
    setMsgToActorId,
    setMsgChannel,
    setMsgTransport,
    setMsgRoute,
    setMsgTemplate,
    setMsgPayload,
    setMsgIdempotencyKey,
    setChatDraft,
    setChatStickToBottom,
    setChatSeenByConversation,
    setInboxActorId,
    setInboxLimit,
    setInboxAfterId,
    setInboxIncludeDelivered,
    setInbox,
    setSelectedMemberId,
    markConversationSeen,
    resetChatSeen,
    dispatch,
  };
}
