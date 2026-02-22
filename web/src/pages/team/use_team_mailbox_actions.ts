import { useCallback } from "react";
import { api, type TeamActorMessageRecord } from "../../api";
import {
  buildMailboxChatPayload,
  type TeamMailboxChatActors,
} from "./mailbox_helpers";
import { parseOptionalJson, parseRequiredJson } from "./create_helpers";
import type { TeamTab } from "./state";

type UseTeamMailboxActionsParams = {
  token: string;
  activeRunId: string | null;
  tab: TeamTab;
  chatActors: TeamMailboxChatActors;
  chatDraft: string;
  msgFromActorId: string;
  msgToActorId: string;
  msgChannel: string;
  msgTransport: "local" | "remote";
  msgRoute: string;
  msgPayload: string;
  msgIdempotencyKey: string;
  inboxActorId: string;
  setBusy: (value: string | null) => void;
  setError: (value: string | null) => void;
  parseErrorMessage: (error: unknown) => string;
  setChatDraft: (value: string) => void;
  loadInbox: (actorIdOverride?: string) => Promise<void>;
  refreshEvents: (runId: string, mode?: "replace" | "prepend") => Promise<unknown>;
  refreshSnapshot: (runId: string) => Promise<unknown>;
};

export function useTeamMailboxActions({
  token,
  activeRunId,
  tab,
  chatActors,
  chatDraft,
  msgFromActorId,
  msgToActorId,
  msgChannel,
  msgTransport,
  msgRoute,
  msgPayload,
  msgIdempotencyKey,
  inboxActorId,
  setBusy,
  setError,
  parseErrorMessage,
  setChatDraft,
  loadInbox,
  refreshEvents,
  refreshSnapshot,
}: UseTeamMailboxActionsParams) {
  const onSendChatMessage = useCallback(async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    const fromActorId = chatActors.fromActorId.trim();
    const toActorId = chatActors.toActorId.trim();
    const text = chatDraft.trim();
    if (!fromActorId || !toActorId) {
      setError("Select a valid member conversation first");
      return;
    }
    if (!text) {
      setError("Chat message is required");
      return;
    }
    setBusy("send-chat");
    setError(null);
    try {
      await api.sendTeamRunMessage(token, activeRunId, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: "default",
        transport: "local",
        payload: buildMailboxChatPayload(text),
      });
      setChatDraft("");
      await refreshSnapshot(activeRunId);
      await loadInbox(toActorId);
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    chatActors.fromActorId,
    chatActors.toActorId,
    chatDraft,
    loadInbox,
    parseErrorMessage,
    refreshSnapshot,
    setBusy,
    setChatDraft,
    setError,
    token,
  ]);

  const onSendMessage = useCallback(async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    const fromActorId = msgFromActorId.trim();
    const toActorId = msgToActorId.trim();
    if (!fromActorId || !toActorId) {
      setError("from_actor_id and to_actor_id are required");
      return;
    }
    setBusy("send-message");
    setError(null);
    try {
      await api.sendTeamRunMessage(token, activeRunId, {
        from_actor_id: fromActorId,
        to_actor_id: toActorId,
        channel: msgChannel.trim() || undefined,
        transport: msgTransport,
        route: parseOptionalJson(msgRoute, "Message route"),
        payload: parseRequiredJson(msgPayload, "Message payload"),
        idempotency_key: msgIdempotencyKey.trim() || undefined,
      });
      if (tab === "mailbox") {
        await refreshSnapshot(activeRunId);
        if (inboxActorId.trim()) {
          await loadInbox();
        }
      } else {
        await Promise.all([refreshEvents(activeRunId), refreshSnapshot(activeRunId)]);
      }
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [
    activeRunId,
    inboxActorId,
    loadInbox,
    msgChannel,
    msgFromActorId,
    msgIdempotencyKey,
    msgPayload,
    msgRoute,
    msgToActorId,
    msgTransport,
    parseErrorMessage,
    refreshEvents,
    refreshSnapshot,
    setBusy,
    setError,
    tab,
    token,
  ]);

  const onRefreshInbox = useCallback(async () => {
    if (!activeRunId) {
      setError("Select a run first");
      return;
    }
    setBusy("refresh-inbox");
    setError(null);
    try {
      await loadInbox();
    } catch (err) {
      setError(parseErrorMessage(err));
    } finally {
      setBusy(null);
    }
  }, [activeRunId, loadInbox, parseErrorMessage, setBusy, setError]);

  const onAckMessage = useCallback(
    async (message: TeamActorMessageRecord) => {
      if (!activeRunId) return;
      const actorId = inboxActorId.trim() || message.to_actor_id;
      setBusy(`ack-${message.message_id}`);
      setError(null);
      try {
        await api.ackTeamRunMessage(token, activeRunId, message.message_id, actorId);
        if (tab === "mailbox") {
          await Promise.all([loadInbox(actorId), refreshSnapshot(activeRunId)]);
        } else {
          await Promise.all([
            loadInbox(),
            refreshEvents(activeRunId),
            refreshSnapshot(activeRunId),
          ]);
        }
      } catch (err) {
        setError(parseErrorMessage(err));
      } finally {
        setBusy(null);
      }
    },
    [
      activeRunId,
      inboxActorId,
      loadInbox,
      parseErrorMessage,
      refreshEvents,
      refreshSnapshot,
      setBusy,
      setError,
      tab,
      token,
    ]
  );

  return {
    onSendChatMessage,
    onSendMessage,
    onRefreshInbox,
    onAckMessage,
  };
}
